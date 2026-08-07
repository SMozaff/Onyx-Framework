# Test reports

`npm run test` writes the Vitest JUnit report to `test-reports/junit.xml`.
That runtime report is intentionally absent from this handoff because the
container registry could not install the frozen npm dependencies. The
`static-validation.xml` report records only checks that were actually run.
