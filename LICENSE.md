
# Slint License

This repository contains several parts under different licenses:

- The **Slint framework** (the runtime libraries and the language tooling) is triple-licensed.
  See below.
- The **documentation and examples** are under the [MIT License](LICENSES/MIT.txt),
  so you can reuse them in any project, including proprietary ones.
- Some **third-party assets and code** keep their own licenses.

The exact license of each file is documented in its header or, where that's not possible, via [REUSE](https://reuse.software/) metadata in `REUSE.toml`.
The full text of every license used is in the `LICENSES` folder.

## Licenses of the Slint Framework

You can use the Slint framework under ***any*** of the following licenses, at your choice:

1. [Royalty-free License](LICENSES/LicenseRef-Slint-Royalty-free-2.0.md) - Permits use in **proprietary** desktop, mobile, and web applications **at no cost**. Use in embedded systems is excluded.
2. [GNU GPLv3](LICENSES/GPL-3.0-only.txt) - Permits use in **open source software** under GPL-compatible terms, **at no cost**, for desktop, mobile, and web applications, as well as for embedded systems.
3. [Commercial license](LICENSES/LicenseRef-Slint-Software-3.0.md) - Permits use in **proprietary** applications, including desktop, mobile, web, and embedded systems.
   See the [pricing page](https://slint.dev/pricing) for the available plans.

### What This Means in Practice

- **Do you want to build an open-source application?**
  Use Slint for free under the GPLv3 on any platform; your own files can stay MIT or Apache-2.0.
  You're not tied to the GPL, though — the Royalty-free or Commercial license works too.
- **Do you want to keep your application proprietary?**
  Both the Royalty-free and Commercial licenses cover proprietary desktop, mobile, and web applications.
  The Royalty-free License is free, as long as you disclose that you use Slint (for example with the `AboutSlint` widget or the Slint badge); without that disclosure, use the Commercial license.
  A Commercial license is required for embedded systems, regardless of disclosure.

The Royalty-free License is meant for applications that run on a user's own general-purpose computer or phone, installed as one application among many.
It does not cover embedded systems — computer systems that perform a specific task within a larger mechanical or electrical system — which need the Commercial license.

See the [Licensing FAQ](FAQ.md#licensing) for the details and common scenarios.

## Definitions

A ***Desktop Application*** is a computer program that is designed to run on a general-purpose computer (PC or notebook), typically installed and executed locally on the computer's operating system.

A ***Mobile Application*** is a computer program that is designed to run on a general-purpose mobile computer (mobile phone or tablet), typically installed and executed locally on the computer's operating system.

A ***Web Application*** is a computer program that is designed to run in the sandbox environment provided by a web browser.

An ***Embedded System*** is a computer system designed to perform a specific task within a larger mechanical or electrical system.

## Contributions

Contributions to this repository are licensed under the [MIT No Attribution License (MIT-0)](https://opensource.org/license/mit-0).
See [CONTRIBUTING.md](CONTRIBUTING.md) for details, including the Contributor License Agreement.

## Additional Info

See the [Slint licensing options on the website](https://slint.dev/pricing) and the [Licensing FAQ](FAQ.md#licensing).
Contact us at [info@slint.dev](mailto:info@slint.dev) if you have any questions regarding licensing.
