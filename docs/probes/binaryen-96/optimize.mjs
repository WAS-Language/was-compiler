import { readFileSync, writeFileSync } from 'node:fs';
import binaryen from 'binaryen';

const [level, input, output, profile] = process.argv.slice(2);
const module = binaryen.readBinary(readFileSync(input));

try {
  module.setFeatures(binaryen.Features.MVP);
  binaryen.setOptimizeLevel(Number(level));
  binaryen.setShrinkLevel(0);
  binaryen.setDebugInfo(false);

  if (!module.validate()) throw new Error('Binaryen rejected the input');
  if (profile === undefined) module.optimize();
  else if (profile === 'local-peephole-v1') module.runPasses(['optimize-instructions']);
  else throw new Error(`Unknown profile: ${profile}`);
  if (!module.validate()) throw new Error('Binaryen rejected the output');

  writeFileSync(output, module.emitBinary());
} finally {
  module.dispose();
}
