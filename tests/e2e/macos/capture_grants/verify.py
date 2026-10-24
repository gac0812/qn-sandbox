import argparse, platform, hashlib, http.server, json, os, pathlib, select, shutil, subprocess, tempfile, threading
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
checks = {'environment_clean' => ENV['QN_FAKE_SECRET'].nil?}
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
    class Upstream(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            received.append(self.headers.get('Authorization') == 'Bearer qn-synthetic-credential')
            self.send_response(200); self.end_headers(); self.wfile.write(b'ok')
        def log_message(self, *args): pass
    upstream = http.server.HTTPServer(('127.0.0.1',0), Upstream)
    thread = threading.Thread(target=upstream.serve_forever); thread.start()
    broker = None
    try:
        for d in ['home','config','state','cache','tmp','project']:(root/d).mkdir()
        relay = root/'capture-relay'; shutil.copy2(BASE/'target/debug/examples/capture_relay', relay)
        (root/'home/.ssh').mkdir()
        (root/'home/.ssh/canary').write_text('synthetic-host-secret')
        (root/'provider-secret').write_text('qn-synthetic-credential')
        os.chmod(root/'provider-secret',0o600)
        binding = root/'binding.json'
        origin = 'http://127.0.0.1:'+str(upstream.server_port)
        broker = subprocess.Popen([str(BASE/'target/debug/examples/grant_capture_probe'),str(binding),origin,mode],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        assert select.select([broker.stdout],[],[],8)[0], 'broker readiness timeout'
        assert broker.stdout.readline().strip() == 'ready', 'broker readiness failed'
        os.chmod(binding,0o600)
        port = int(json.loads(binding.read_text())['address'].split(':')[-1])
        (root/'project/client.rb').write_text(CLIENT)
        profile = {'meta':{'name':'qn-capture-test','version':'1'},'extends':'default','filesystem':{'allow':[str(root/'project')]},'security':{'capability_elevation':False,'signal_mode':'isolated','process_info_mode':'isolated'},'environment':{'allow_vars':['PATH','HOME','LANG']},'network':{'credentials':['qnprobe'],'custom_credentials':{'qnprobe':{'upstream':origin,'credential_key':'cmd://qnprobe','env_var':'QNPROBE_TOKEN','inject_header':'Authorization','credential_format':'Bearer {}','endpoint_rules':[{'method':'GET','path':'/probe'}]}}},'credential_capture':{'qnprobe':{'provider':{'command':[str(relay),str(binding)],'config':{}},'stdin':'request_json','cache_ttl_secs':0,'timeout_secs':6}}}
        policy=root/'profile.json';policy.write_text(json.dumps(profile));os.chmod(policy,0o600)
        env={'PATH':'/usr/bin:/bin','LANG':'C','QN_FAKE_SECRET':'synthetic-parent-secret'}
        for k,d in [('HOME','home'),('XDG_CONFIG_HOME','config'),('XDG_STATE_HOME','state'),('XDG_CACHE_HOME','cache'),('TMPDIR','tmp')]:env[k]=str(root/d)
        def run(*args):
            p = subprocess.run([str(BASE/'target/debug/examples/probe'),str(NONO),*map(str,args)],cwd=root/'project',env=env,capture_output=True,text=True,timeout=25)
            assert p.returncode==0,'infra launcher failed'
            result=json.loads(p.stdout)
            assert not result['stdout_truncated'] and not result['stderr_truncated']
            return result
        valid=run('profile','validate','--json','--strict','--',policy)
        assert valid['exit_code']==0,'profile rejected'
        result=run('run','--profile',policy,'--workdir',root/'project','--diagnostics-json','--','/usr/bin/ruby',root/'project/client.rb',binding,port,1 if mode=='race' else 2)
        print(json.dumps({'mode':mode,'exit_code':result['exit_code'],'child_stdout':result['stdout'],'upstream_requests':len(received),'auth_matches':received}),flush=True)
        lines=result['stdout'].splitlines()
        assert result['exit_code']==0, 'sandbox client failed'
        assert lines and json.loads(lines[0])=={'provider_secret_denied':True,'binding_denied':True,'broker_connect_denied':True,'placeholder':True,'environment_clean':True,'host_secret_denied':True},'sandbox boundary failed'
        assert lines[1:]==(['503'] if mode=='race' else ['200','503']), 'request/revoke failed'
        assert received==([] if mode=='race' else [True]), 'unexpected upstream request'
        out,err=broker.communicate(timeout=8)
        print('broker:',out.strip(),'exit:',broker.returncode,flush=True)
        if err:print('broker:',err.strip(),flush=True)
        assert broker.returncode==0
        assert 'qn-synthetic-credential' not in out+err, 'broker logged credential material'
        assert 'qn-synthetic-credential' not in result['stdout']+result['stderr'], 'credential leaked into process output'
    finally:
        if broker and broker.poll() is None:broker.terminate();broker.communicate(timeout=8)
        upstream.shutdown();upstream.server_close();thread.join(timeout=3)
        shutil.rmtree(root)
for mode in ['normal','race']:case(mode)
