"""Synthetic supervisor-only capture probe, with no real credentials (#46)."""
import http.server
import json
import threading


def probe_capture(root, project, base, profile, invoke, run):
    revoked = root / 'revoked'
    received = []

    class Upstream(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            received.append(self.headers.get('Authorization') == 'Bearer qn-synthetic-credential')
            revoked.write_text('revoked')
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'ok')

        def log_message(self, *args):
            pass

    server = http.server.HTTPServer(('127.0.0.1', 0), Upstream)
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    provider = root / 'provider.rb'
    provider.write_text('''require 'json'
request = JSON.parse(STDIN.read)
exit 1 if File.exist?(request.fetch('config').fetch('revoked'))
puts JSON.generate(material: {type: 'secret', value: 'qn-synthetic-credential'})
''')
    client = project / 'client.rb'
    client.write_text('''require 'net/http'
require 'json'
key = ENV.fetch('QNPROBE_TOKEN')
puts JSON.generate(placeholder: !key.empty? && key != 'qn-synthetic-credential')
url = URI(ENV.fetch('QNPROBE_BASE_URL') + '/probe')
2.times do
  http = Net::HTTP.new(url.host, url.port, nil)
  http.open_timeout = 3
  http.read_timeout = 5
  request = Net::HTTP::Get.new(url)
  request['Authorization'] = 'Bearer ' + key
  puts http.request(request).code
end
''')
    policy = json.loads(json.dumps(base))
    policy['network'] = {'credentials': ['qnprobe'], 'custom_credentials': {'qnprobe': {
        'upstream': 'http://127.0.0.1:' + str(server.server_port),
        'credential_key': 'cmd://qnprobe', 'env_var': 'QNPROBE_TOKEN',
        'inject_header': 'Authorization', 'credential_format': 'Bearer {}',
        'endpoint_rules': [{'method': 'GET', 'path': '/probe'}]}}}
    policy['credential_capture'] = {'qnprobe': {'provider': {
        'command': ['/usr/bin/ruby', str(provider)], 'config': {'revoked': str(revoked)}},
        'stdin': 'request_json', 'timeout_secs': 5, 'cache_ttl_secs': 0}}
    try:
        path = profile('capture', policy)
        invoke('capture_validate', 'profile', 'validate', '--json', '--strict', '--', path)
        result = run('capture', path, '/usr/bin/ruby', str(client))
        # Never print the phantom or the received authentication header.
        summary = {'upstream_requests': len(received), 'correct_auth': received,
                   'revoked': revoked.exists(), 'child_stdout': result['stdout']}
        (root / 'capture-summary.json').write_text(json.dumps(summary, indent=2))
        print('Capture summary:', json.dumps(summary), flush=True)
        assert summary['upstream_requests'] == 1 and summary['correct_auth'] == [True]
        assert result['stdout'] == '{"placeholder":true}\n200\n503\n'
        denied = run('provider_inaccessible', path, '/usr/bin/ruby', str(provider))
        assert denied['exit_code'] != 0 and not denied['stdout']
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
