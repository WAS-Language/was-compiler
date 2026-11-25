import { readFileSync, writeFileSync } from 'node:fs';
import binaryen from 'binaryen';

const [level, input, output] = process.argv.slice(2);
const module = binaryen.readBinary(readFileSync(input));

try {
  module.setFeatures(binaryen.Features.MVP);
  binaryen.setOptimizeLevel(Number(level));
  binaryen.setShrinkLevel(0);
  binaryen.setDebugInfo(false);

  if (!module.validate()) throw new Error('Binaryen rejected the input');
  module.optimize();
  if (!module.validate()) throw new Error('Binaryen rejected the output');

  writeFileSync(output, module.emitBinary());
} finally {
  module.dispose();
}
