# Validation guide

After core publication, run from the ecosystem root:

```bash
nix develop -c scripts/gates.sh
nix develop -c scripts/release-check.sh
```

The gates fetch the exact declared release and collect binding/model tests. Invocation and
command tests compare release fixture bytes, altered replay, invalid cursors, duplicate keys
and legacy required-governance refusal with unchanged history. Release-check installs outside
the checkout and compares the installed package with the in-repository build.

behavior engine-info and behavior.versions() must agree on core0.12.0 and its exact commit,
accepted Wire0.8, record0.7, read-recordv2 and verifier0.8.0. The packaged CLI adds invocation
and authenticated governance. Checked wire documents provide current command authoring.
