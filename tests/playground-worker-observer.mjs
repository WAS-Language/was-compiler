// Browser-only test bootstrap. It runs the unmodified production worker after
// installing a tap at its published-artifact / guest-instantiation boundary.
const post = self.postMessage.bind(self);
const instantiate = WebAssembly.instantiate;
let pending = false;
self.postMessage = (value, transfer) => {
    if (value.type === 'compiled') pending = true;
    post(value, transfer);
};
WebAssembly.instantiate = (source, imports) => {
    if (pending) {
        pending = false;
        post({ type: 'observed-instantiation', bytes: source });
    }
    return instantiate(source, imports);
};
