import json
import subprocess
from pathlib import Path

REPO = 'WAS-Language/was-compiler'
ROOT = Path(__file__).resolve().parent
STATE = ROOT / 'published.json'
TICKETS = [
    ('01-allocator-core.md', 'M4: Extract the reusable allocator core without activation', []),
    ('02-constructor.md', 'M4: Implement the explicit resizable-array copy constructor', [1]),
    ('03-reserve-and-safe-writes.md', 'M4: Implement reserve with relocation-safe indexed writes', [2]),
    ('04-push.md', 'M4: Implement push across spare-capacity and growth paths', [3]),
    ('05-pop-remove.md', 'M4: Implement pop and order-preserving remove', [3]),
    ('06-adapters.md', 'M4: Prove CLI, browser and raw-host compatibility', [4, 5]),
    ('07-failure-composition.md', 'M4: Verify authority and failure recovery under composition', [4, 5]),
    ('08-measurements.md', 'M4: Measure growth, reuse and retained-storage costs', [7]),
    ('09-integrated-release.md', 'M4: Run integrated gates and record release readiness', [6, 8]),
]


def run(args):
    result = subprocess.run(args, text=True, encoding='utf-8', capture_output=True, timeout=45)
    if result.returncode:
        raise RuntimeError(f'{args!r}\n{result.stdout}\n{result.stderr}')
    return result.stdout.strip()


def api(endpoint, *args):
    return json.loads(run(['gh', 'api', endpoint, *args]))


def save(state):
    STATE.write_text(json.dumps(state, indent=2) + '\n', encoding='utf-8')


for filename, title, blockers in TICKETS:
    body = (ROOT / 'issues' / filename).read_text(encoding='utf-8')
    for section in ('## What to build', '## Acceptance criteria', '## Implementation handoff',
                    '**Main files', '**Read first', '**Validation', '**Suggested model',
                    '**Thinking effort', '**Rationale', '**Escalation trigger'):
        assert section in body, (filename, section)
    assert len(body) < 65536, filename

state = json.loads(STATE.read_text(encoding='utf-8')) if STATE.exists() else {}
for index, (filename, title, blockers) in enumerate(TICKETS, 1):
    key = str(index)
    if key not in state:
        matches = api(f'repos/{REPO}/issues?state=open&per_page=100')
        exact = [issue for issue in matches if issue.get('title') == title and 'pull_request' not in issue]
        if exact:
            raise RuntimeError(f'Existing matching issue needs manual reconciliation: {exact[0]["html_url"]}')
        url = run(['gh', 'issue', 'create', '--repo', REPO, '--title', title,
                   '--label', 'ready-for-agent', '--body-file', str(ROOT / 'issues' / filename)])
        number = int(url.rstrip('/').split('/')[-1])
        state[key] = {'number': number, 'url': url, 'title': title, 'blocked_by': []}
        save(state)
        print(f'Created {index}: {url}', flush=True)
    entry = state[key]
    issue = api(f'repos/{REPO}/issues/{entry["number"]}')
    assert issue['title'] == title
    assert 'ready-for-agent' in [label['name'] for label in issue['labels']]
    entry['id'] = issue['id']
    save(state)
    endpoint = f'repos/{REPO}/issues/{entry["number"]}/dependencies/blocked_by'
    current = api(endpoint)
    current_numbers = {item['number'] for item in current}
    expected = {state[str(blocker)]['number'] for blocker in blockers}
    assert current_numbers <= expected, (entry['number'], current_numbers, expected)
    for blocker in blockers:
        source = state[str(blocker)]
        if source['number'] not in current_numbers:
            api(endpoint, '--method', 'POST', '-F', f'issue_id={source["id"]}')
            print(f'Linked #{entry["number"]} blocked by #{source["number"]}', flush=True)
    verified = api(endpoint)
    assert {item['number'] for item in verified} == expected
    remote = api(f'repos/{REPO}/issues/{entry["number"]}')
    assert remote['body'].replace('\r\n', '\n').strip() == (ROOT / 'issues' / filename).read_text(encoding='utf-8').strip()
    entry['blocked_by'] = sorted(expected)
    entry['verified'] = True
    save(state)

print(json.dumps(state, indent=2), flush=True)
