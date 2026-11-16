"""Read-only verification of the approved M5 GitHub publication."""
import json
from pathlib import Path
import subprocess

REPO = "WAS-Language/was-compiler"
ISSUES = {
    73: ("01-concat-byte-length.md", []),
    74: ("02-byte-slicing.md", [73]),
    75: ("03-replace-all.md", [73]),
    76: ("04-immutable-adapter-gate.md", [74, 75]),
    77: ("05-fragment-join.md", [73]),
    78: ("06-private-observations.md", [73]),
    79: ("07-linear-construction.md", [77, 78]),
    80: ("08-ordinary-measurements.md", [75, 77]),
    81: ("09-integrated-acceptance.md", [76, 79, 80]),
}


def gh_json(*args):
    result = subprocess.run(
        ["gh", *args], check=True, capture_output=True, text=True,
        encoding="utf-8", timeout=20,
    )
    return json.loads(result.stdout)


def normalized(body):
    return body.replace("\r\n", "\n").strip()


published = gh_json(
    "issue", "list", "--repo", REPO, "--state", "open", "--limit", "100",
    "--json", "number,title,body,labels,url",
)
by_number = {issue["number"]: issue for issue in published}
for number, (filename, blockers) in ISSUES.items():
    issue = by_number[number]
    assert "ready-for-agent" in {label["name"] for label in issue["labels"]}, number
    local = Path(__file__).with_name(filename).read_text(encoding="utf-8")
    assert normalized(issue["body"]) == normalized(local), (number, "body mismatch")
    actual = sorted(item["number"] for item in gh_json(
        "api", f"repos/{REPO}/issues/{number}/dependencies/blocked_by",
    ))
    assert actual == blockers, (number, actual, blockers)
    print(f"#{number}: body/label verified; blockers={actual}; {issue['title']}")
print("Verified all 9 issue bodies, ready-for-agent labels and native blocking edges.")
