# Contributing

We warmly welcome contributions to the project. Let's discuss ideas or questions
in [Github discussions](https://github.com/slint-ui/slint/discussions) or on our [public chat](https://chat.slint.dev).
Please feel welcome to open GitHub issues or pull requests.
Use 👍 reaction on issues that you consider important.

Issues which we think are suitable for new contributors are tagged with
https://github.com/slint-ui/slint/labels/good%20first%20issue.

If you use an AI coding assistant, it reads [AGENTS.md](AGENTS.md) for build commands and
architecture notes specific to this repository.

## Pull Requests

Keep pull requests small and focused.
A bug fix or one self-contained change is easy to review; a large code drop isn't, and may wait a long time.
If you plan a big feature, discuss it first in an issue or on our [public chat](https://chat.slint.dev) before writing the code.
Slint is 1.x and has a stable API, so discuss any public API change with the Slint team first.
That way we agree on the design early, and your contribution is much more likely to be accepted.

## Internal documentation

 - [Development guide](docs/development.md)
 - [Building Slint from sources in this repository](docs/building.md)
 - [Testing](docs/testing.md)
 - [GitHub issues triage and labels](docs/internal/triage.md)
 - [Writing style guide](docs/internal/writing-style-guide.md) for commit messages, code comments, and documentation

## License

By contributing to this project, you agree to license your contributions under
the [MIT No Attribution license (MIT-0)](https://opensource.org/license/mit-0).
This doesn't assign copyright or transfer ownership:
you keep full rights to your code, and stay free to reuse it
in other projects under any license you choose.

Make sure you wrote the contribution yourself, and that no rights
have been transferred to a third party, such as your employer.
If that isn't the case, let us know before opening the pull request.

You confirm this once, for all your contributions.
When you open your first pull request, a bot adds a checkbox to its description.
Tick it to accept these terms; you aren't asked again in later pull requests.
Pull requests from accounts that haven't accepted the terms can't be merged.

### Our Open-Source Pledge

We believe that open-source software development and communities are the foundation
for a healthy ecosystem of high-quality software,
where everyone can learn, improve and give back.
We commit to upholding this foundation and pledge by promising to continue
to develop Slint in the open under an open-source license compliant with the [Open Source Definition](https://opensource.org/osd).

Further, we commit to provide a royalty-free license
for those who develop desktop, mobile, or web applications
and do not want to use open-source components under copyleft licenses.

## Coding Style

For the Rust portion of the code base, the CI enforces the coding style via rustfmt.
For the C++ portion of the code base, the CI enforces the coding style via `clang-format`.

