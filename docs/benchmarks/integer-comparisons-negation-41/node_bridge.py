"""Pass the recorded payload to the unchanged Node stdin boundary, with a deadline."""
from pathlib import Path
import subprocess
import sys

result = subprocess.run(["node", str(Path(__file__).with_name("expression_measure.cjs")), sys.argv[1]],
    input=Path(sys.argv[2]).read_bytes(), timeout=60, check=False)
sys.exit(result.returncode)
