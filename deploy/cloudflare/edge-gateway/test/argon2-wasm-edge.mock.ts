export { argon2id, argon2Verify } from "hash-wasm";

export function setWASMModules(_modules: { argon2WASM: unknown; blake2bWASM: unknown }): void {
  // Unit tests run under Node. The real statically compiled modules are exercised
  // by the isolated Wrangler/Workerd authentication smoke workflow.
}
