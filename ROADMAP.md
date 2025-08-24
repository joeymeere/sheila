# Sheila - Improvement Roadmap

### **CargoTestRunner CLI Integration**
**Problem A**: CLI is handling much of the machinery for running tests, which we want to offload to the core library.
**Problem**: `CargoTestRunner` is incomplete and doesn't integrate with CLI yet

**Solution**:
- Port CLI test execution logic to `CargoTestRunner`
- Implement proper process management
- Add build step integration
- Support for different cargo profiles

### **Reporting Improvements**
- Test progress updates in the CLI
- CI/CD mode
- Better HTML reports with filtering & page templates

### **Type-Safe Test Context**
**Problem A**: `TestContext` isn't currently usable within a test.
**Problem B**: `TestContext` uses `serde_json::Value` instead of type-safe generics.

**Solution**:
```rust
// Target API:
#[sheila::test]
fn test_with_typed_params(ctx: TestContext<MyParams>) {
    let params: MyParams = ctx.params();
}
```

### **Debugger & IDE Integration**
- Better tooling for debugging in the CLI
- VSCode Debugger Integration

### **Fixture System Tweaks**
- Support async fixtures