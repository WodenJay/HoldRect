# Contributing

Thanks for your interest in HoldRect! Here's how to contribute.

## Ground Rules

1. **One PR, one thing.** Keep each pull request focused on a single change — one bug fix, one feature, one refactor. Easier to review, easier to revert.

2. **Test-Driven Development.** Write the test first, watch it fail, then write the code to make it pass. PRs without tests will not be merged.

3. **All tests must pass.** Run `cargo test` before submitting. CI must be green.

4. **Discuss before you build.** For anything other than a bug fix, open an issue first. Describe what you want to change and why. Get the green light before writing code.

## Development Setup

```bash
git clone https://github.com/WodenJay/HoldRect.git
cd HoldRect
cargo build
cargo test
```

**Requirements:** Rust 1.75+, Windows 10+

## Code Quality

Before submitting a PR, make sure:

```bash
cargo clippy -- -D warnings  # no clippy warnings
cargo test              # all tests pass
```

## Commit Messages

- Do not start commit messages with `@`
- Keep them short and descriptive
- Format: `type(scope): description` (e.g. `fix(render): correct border offset on HiDPI`)

## Reporting Bugs

Open an issue with:
- What you expected
- What actually happened
- Steps to reproduce
- Your OS and Rust version

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
