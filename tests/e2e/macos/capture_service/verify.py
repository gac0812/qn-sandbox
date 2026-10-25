import socket, argparse, platform, hashlib, http.server, json, os, pathlib, select, shutil, subprocess, tempfile, threading
parser = argparse.ArgumentParser(description='Synthetic pinned-nono grant capture verification (#99)')
parser.add_argument('--workspace', required=True, type=pathlib.Path)
parser.add_argument('--nono', required=True, type=pathlib.Path)
args = parser.parse_args()
assert platform.system() == 'Darwin' and platform.machine() == 'arm64', 'verified arm64 macOS fixture'
BASE = args.workspace.resolve()
NONO = args.nono.resolve()
assert hashlib.sha256(NONO.read_bytes()).hexdigest() == 'e95479a2ef64b19e3245a90a60024bb248e96e8ec234ffeb25e062ddd45b1417'
CLIENT = r'''require 'net/http'
require 'json'
require 'socket'
binding = ARGV.fetch(0)
port = Integer(ARGV.fetch(1))
checks = {'environment_clean' => ENV['QN_FAKE_SECRET'].nil? && ENV['NONO_DETACHED_SESSION_ID'].nil?}
begin
  File.read(File.join(ENV.fetch('HOME'), '.ssh/canary'))
  checks['host_secret_denied'] = false
rescue Errno::EACCES, Errno::EPERM
  checks['host_secret_denied'] = true
end
begin
  File.read(binding)
  checks['binding_denied'] = false
rescue Errno::EACCES, Errno::EPERM
  checks['binding_denied'] = true
end
begin
  socket = Socket.tcp('127.0.0.1', port, connect_timeout: 1)
  socket.close
  checks['broker_connect_denied'] = false
rescue SystemCallError, IOError
  checks['broker_connect_denied'] = true
end
begin
  File.read(File.join(File.dirname(binding), 'provider-secret'))
  checks['provider_secret_denied'] = false
rescue Errno::EACCES, Errno::EPERM
  checks['provider_secret_denied'] = true
end
begin
  File.read(File.join(File.dirname(binding), 'broker-audit.jsonl'))
  checks['audit_denied'] = false
rescue Errno::EACCES, Errno::EPERM
  checks['audit_denied'] = true
end
key = ENV.fetch('QNPROBE_TOKEN')
checks['placeholder'] = !key.empty? && key != 'qn-synthetic-credential'
puts JSON.generate(checks)
url = URI(ENV.fetch('QNPROBE_BASE_URL') + '/probe')
Integer(ARGV.fetch(2)).times do
  http = Net::HTTP.new(url.host, url.port, nil)
  http.open_timeout = 3
  http.read_timeout = 7
  request = Net::HTTP::Get.new(url)
  request['Authorization'] = 'Bearer ' + key
  puts http.request(request).code
end
'''
def case(mode):
    root = pathlib.Path(tempfile.mkdtemp(prefix='qn-96-', dir='/Users/Shared'))
    os.chmod(root, 0o700)
    received = []
    def control(command):
        host,port=(root/'control-address').read_text().split(':')
        with socket.create_connection((host,int(port)),timeout=3) as channel:
            channel.sendall(command)
            assert channel.recv(1)==b'A'
    class Upstream(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            received.append(self.headers.get('Authorization') == 'Bearer qn-synthetic-credential')
            control(b'R')
            self.send_response(200); self.end_headers(); self.wfile.write(b'ok')
        def log_message(self, *args): pass
    upstream = http.server.HTTPServer(('127.0.0.1',0), Upstream)
    thread = threading.Thread(target=upstream.serve_forever); thread.start()
    broker = None
    launcher = None
    try:
        for d in ['home','config','state','cache','tmp','project']:(root/d).mkdir()
        relay = root/'capture-relay'; shutil.copy2(BASE/'target/debug/examples/capture_relay', relay)
        (root/'home/.ssh').mkdir()
        (root/'home/.ssh/canary').write_text('synthetic-host-secret')
        (root/'provider-secret').write_text('qn-synthetic-credential')
        os.chmod(root/'provider-secret',0o600)
        binding = root/'binding.json'
        origin = 'http://127.0.0.1:'+str(upstream.server_port)
        launcher_env = dict(os.environ, QN_FAKE_SECRET='synthetic-parent-secret', NONO_DETACHED_SESSION_ID='bad-parent-identity')
        launcher = subprocess.Popen([str(BASE/'target/debug/examples/bound_launch_probe'),str(root),str(NONO)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=launcher_env)
        assert select.select([launcher.stdout],[],[],8)[0], 'launch reservation timeout'
        association = json.loads(launcher.stdout.readline())
        native = association['native_id']
        expected_native = native if mode != 'mismatch' else format(int(native,16)^1,'016x')
        broker = subprocess.Popen([str(BASE/'target/debug/examples/grant_capture_probe'),str(binding),origin,mode,expected_native,association['session_id']],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        assert select.select([broker.stdout],[],[],8)[0], 'broker readiness timeout'
        assert broker.stdout.readline().strip() == 'ready', 'broker readiness failed'
        os.chmod(binding,0o600)
        port = int(json.loads(binding.read_text())['address'].split(':')[-1])
        (root/'project/client.rb').write_text(CLIENT)
        profile = {'meta':{'name':'qn-capture-test','version':'1'},'extends':'default','filesystem':{'allow':[str(root/'project')]},'security':{'capability_elevation':False,'signal_mode':'isolated','process_info_mode':'isolated'},'environment':{'allow_vars':['PATH','HOME','LANG']},'network':{'credentials':['qnprobe'],'custom_credentials':{'qnprobe':{'upstream':origin,'credential_key':'cmd://qnprobe','env_var':'QNPROBE_TOKEN','inject_header':'Authorization','credential_format':'Bearer {}','endpoint_rules':[{'method':'GET','path':'/probe'}]}}},'credential_capture':{'qnprobe':{'provider':{'command':[str(relay),str(binding)],'config':{}},'stdin':'request_json','cache_ttl_secs':0,'timeout_secs':6}}}
        policy=root/'profile.json';policy.write_text(json.dumps(profile));os.chmod(policy,0o600)
        out,err = launcher.communicate(json.dumps(['/usr/bin/ruby',str(root/'project/client.rb'),str(binding),str(port),str(2 if mode=='normal' else 1)]), timeout=25)
        assert launcher.returncode == 0, 'bound launcher failed: ' + err
        result = json.loads(out)
        assert not result['stdout_truncated'] and not result['stderr_truncated']
        print(json.dumps({'mode':mode,'exit_code':result['exit_code'],'child_stdout':result['stdout'],'upstream_requests':len(received),'auth_matches':received}),flush=True)
        lines=result['stdout'].splitlines()
        assert result['exit_code']==0, 'sandbox client failed'
        assert lines and json.loads(lines[0])=={'audit_denied':True,'provider_secret_denied':True,'binding_denied':True,'broker_connect_denied':True,'placeholder':True,'environment_clean':True,'host_secret_denied':True},'sandbox boundary failed'
        assert lines[1:]==(['200','503'] if mode=='normal' else ['503']), 'request/revoke failed'
        assert received==([True] if mode=='normal' else []), 'unexpected upstream request'
        if mode != 'audit_failure':control(b'Q')
        out,err=broker.communicate(timeout=8)
        print('broker:',out.strip(),'exit:',broker.returncode,flush=True)
        if err:print('broker:',err.strip(),flush=True)
        assert broker.returncode==0
        audit=(root/'broker-audit.jsonl').read_text()
        assert 'qn-synthetic-credential' not in audit
        records=[json.loads(line) for line in audit.splitlines()]
        assert all(record['session_id']==association['session_id'] for record in records), 'audit belongs to another qn session'
        events=[record['event'] for record in records]
        expected={'normal':['grant_issued','provider_requested','delivery_prepared','reply_written','grant_revoked','denied','service_closed'], 'race':['grant_issued','provider_requested','grant_revoked','delivery_prepared','denied','service_closed'], 'mismatch':['grant_issued','denied','grant_revoked','service_closed'], 'audit_failure':['grant_issued','provider_requested']}
        alternate = expected[mode].copy()
        if mode == 'normal':
            alternate[3:5] = ['grant_revoked', 'reply_written']
        assert events in [expected[mode], alternate],events
        print('audit:',events,flush=True)
        assert 'qn-synthetic-credential' not in out+err, 'broker logged credential material'
        assert 'qn-synthetic-credential' not in result['stdout']+result['stderr'], 'credential leaked into process output'
    finally:
        if launcher and launcher.poll() is None:launcher.terminate();launcher.communicate(timeout=8)
        if broker and broker.poll() is None:broker.terminate();broker.communicate(timeout=8)
        upstream.shutdown();upstream.server_close();thread.join(timeout=3)
        shutil.rmtree(root)
for mode in ['normal','race','mismatch','audit_failure']:case(mode)
