export async function postProcess(input, level, showWat) {
    let binaryen, guard;
    try {
        ({ default: binaryen } = await import("./binaryen-116.mjs"));
        guard = await import("./binaryen-check/was_compiler.js");
        await guard.default();
    } catch (error) {
        throw new Error(`Could not load the optional Binaryen 116 assets. See docs/browser.md for setup. ${error}`);
    }

    const module = binaryen.readBinary(input);
    try {
        module.setFeatures(binaryen.Features.MVP);
        binaryen.setOptimizeLevel(level);
        binaryen.setShrinkLevel(0);
        binaryen.setDebugInfo(false);
        if (!module.validate()) throw new Error("Binaryen rejected the compiler output.");
        module.runPasses(["optimize-instructions"]);
        if (!module.validate()) throw new Error("Binaryen produced an invalid module.");
        const bytes = module.emitBinary();
        const wat = guard.checkBinaryenOutput(input, bytes, showWat) ?? null;
        return { bytes, wat };
    } finally {
        module.dispose();
    }
}
