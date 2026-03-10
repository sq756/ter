(function() {
    const implementors = Object.fromEntries([["ed25519",[]],["ed25519_dalek",[]],["pkcs8",[]]]);
    if (window.register_implementors) {
        window.register_implementors(implementors);
    } else {
        window.pending_implementors = implementors;
    }
})()
//{"start":59,"fragment_lengths":[14,21,13]}