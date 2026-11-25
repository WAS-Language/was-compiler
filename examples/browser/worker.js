import init, { Compiler } from "../../pkg/was_compiler.js";

async function postProcess(input, level, showWat) {
    let binaryen, guard;
    try {
        ({ default: binaryen } = await import("./binaryen-116.mjs"));
        guard = await import("./binaryen-check/was_compiler.js");
        await guard.default();
    } catch (error) {
        throw new Error(`Could not load the optional Binaryen 116 assets. See docs/browser.md for setup, or uncheck Binaryen. ${error}`);
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

self.addEventListener("message", async ({ data }) => {
    const logs = [];
    let logsTruncated = false;
    let phase = "Compiler initialization";

    try {
        await init();
        phase = "Compilation";
        const level = data.optimization ?? 1;
        const optimize = data.binaryen === true && level !== 0;
        const compiler = new Compiler();
        let bytes;
        let wat = null;

        try {
            compiler.setExports(["main"]);
            compiler.setOptimizationLevel(level);
            if (data.hostPrint) {
                compiler.registerHostFunction("print", "env", "log_number", ["number"], "void");
            }
            bytes = compiler.compile(data.source).slice();
            if (!optimize && data.showWat) {
                wat = compiler.compileWat(data.source);
            }
        } finally {
            compiler.free();
        }

        if (optimize) {
            phase = "Binaryen post-processing";
            ({ bytes, wat } = await postProcess(bytes, level, data.showWat));
        }

        const downloadable = bytes.slice();
        self.postMessage({ type: "compiled", bytes: downloadable, wat }, [downloadable.buffer]);

        phase = "Instantiation";
        const imports = data.hostPrint ? {
            env: {
                log_number(value) {
                    if (logs.length < 1000) {
                        logs.push(String(value));
                    } else {
                        logsTruncated = true;
                    }
                },
            },
        } : {};
        const { instance } = await WebAssembly.instantiate(bytes, imports);

        phase = "Execution";
        const main = instance.exports.main;
        if (typeof main !== "function" || main.length !== 0) {
            throw new Error("The demo requires an exported zero-argument main() function.");
        }
        const result = main();
        if (typeof result !== "number") {
            throw new Error("The demo requires main() to return a number.");
        }
        self.postMessage({ type: "result", result: String(result), logs, logsTruncated });
    } catch (error) {
        self.postMessage({
            type: "error",
            phase,
            message: error instanceof Error ? error.message : String(error),
            logs,
            logsTruncated,
        });
    }
});
