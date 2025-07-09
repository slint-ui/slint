<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Frequently Asked Questions: <!-- omit in toc -->

- [General](#general)
  - [Why does Slint use a domain specific language?](#why-does-slint-use-a-domain-specific-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [Using Slint in Permissively Licensed Open Source Projects](#using-slint-in-permissively-licensed-open-source-projects)
  - [Royalty-free license](#royalty-free-license)
    - [Who can use the Royalty-free license?](#who-can-use-the-royalty-free-license)
    - [What obligations do I need to fulfil to use the Royalty-free license?](#what-obligations-do-i-need-to-fulfil-to-use-the-royalty-free-license)
    - [Are there any limitations with the Royalty-free license?](#are-there-any-limitations-with-the-royalty-free-license)
    - [Scenario: What happens if my application is open-source (e.g. under MIT), forked by a different person and then redistributed?](#scenario-what-happens-if-my-application-is-open-source-eg-under-mit-forked-by-a-different-person-and-then-redistributed)
    - [How are modifications to Slint itself covered under this license?](#how-are-modifications-to-slint-itself-covered-under-this-license)
    - [If Slint were to be taken over by a larger company or the current owners were to have a change of heart, can they revoke existing licenses?](#if-slint-were-to-be-taken-over-by-a-larger-company-or-the-current-owners-were-to-have-a-change-of-heart-can-they-revoke-existing-licenses)
  - [GPLv3](#gplv3)
    - [If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?](#if-i-link-my-program-with-slint-gplv3-does-it-mean-that-i-have-to-license-my-program-under-the-gplv3-too)
    - [My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?](#my-mit-licensed-program-links-to-slint-gplv3-can-someone-fork-my-program-to-build-and-distribute-a-proprietary-program)
    - [My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?](#my-mit-licensed-program-links-to-slint-gplv3-how-can-i-convey-to-someone-that-they-can-distribute-my-program-as-part-of-a-proprietary-licensed-program)
    - [My MIT-licensed program links to Slint GPLv3. Under what license can I release the binary of my program?](#my-mit-licensed-program-links-to-slint-gplv3-under-what-license-can-i-release-the-binary-of-my-program)
    - [Scenario: Alice is a software developer, she wants her code to be licensed under MIT. She is developing an application "AliceApp" that links to Slint GPLv3. Alice also wants to allow that Bob, a user of AliceApp, can fork AliceApp into a proprietary application called BobApp](#scenario-alice-is-a-software-developer-she-wants-her-code-to-be-licensed-under-mit-she-is-developing-an-application-aliceapp-that-links-to-slint-gplv3-alice-also-wants-to-allow-that-bob-a-user-of-aliceapp-can-fork-aliceapp-into-a-proprietary-application-called-bobapp)
      - [Can Alice use the MIT license header to the source code of AliceApp application?](#can-alice-use-the-mit-license-header-to-the-source-code-of-aliceapp-application)
      - [Under what license should she distribute the AliceApp binary?](#under-what-license-should-she-distribute-the-aliceapp-binary)
      - [How can Alice make it clear to Bob that he can distribute BobApp under a proprietary license?](#how-can-alice-make-it-clear-to-bob-that-he-can-distribute-bobapp-under-a-proprietary-license)
  - [Paid License](#paid-license)
    - [What are the paid license options?](#what-are-the-paid-license-options)
- [Miscellaneous](#miscellaneous)
  - [Do you provide Support?](#do-you-provide-support)

## General

### Why does Slint use a domain specific language?

From our long experience of building UI toolkits, we have learnt that a domain
specific, declarative language is best suited to describe UIs. The Slint language
is easy and intuitive to use while being strict enough for our tools to analyze
and optimize to provide high graphics performance. Strictly typed binding
expressions offer a powerful and robust way for humans to declare relationships
between properties, even in complex user interfaces.

### Will there be API bindings to integrate with my favorite programming language?

We want to make it possible to use Slint with any programming language. We do
not favor one programming language over another. We currently support the following
languages:

- Rust, our implementation language,
- C++, another systems programming language we have a lot of experience with,
- JavaScript, a popular dynamically typed language,
- Python, a popular scripting language.

This choice builds the foundation that allows us to create bindings for most
types of programming languages.

## Licensing

You can use Slint under ***any*** of the following licenses, at your choice:

1. [Royalty-free license](LICENSES/LicenseRef-Slint-Royalty-free-2.0.md),
2. [GNU GPLv3](LICENSES/GPL-3.0-only.txt),
3. [Paid license](LICENSES/LicenseRef-Slint-Software-3.0.md).

### Using Slint in Permissively Licensed Open Source Projects

This guide helps you understand how to use Slint with a permissive license (e.g., MIT, Apache-2.0) while complying with the appropriate Slint license. The key factors to consider are the type of application you’re building, the license of your distributables, the impact on derivative works, and any applicable restrictions.

| Type of Project | Project License | Selected Slint License  | License of the Distributables | Your Obligations | Impact on Derivative Works | Restrictions |
|--|--|--|--|--|--|--|--|
| **Desktop, Mobile, Web** Application | MIT, Apache-2.0, etc. | Royalty-Free | MIT, Apache-2.0, etc. | ✅ Display the [`AboutSlint` widget](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/misc/aboutslint/) in the app's "About" screen or splash screen; *or* display the [Slint badge](https://github.com/slint-ui/slint/tree/master/logo/MadeWithSlint-logo-whitebg.png) on your public download page. | ❗ The Derivate works can use Slint under any one of the licenses - (1) Royalty-free, (2) GPLv3, or (3) Commercial. | ❌ Slint source code may not be redistributed except under GPLv3.  |
| **Embedded Systems, Desktop, Mobile, Web** Application | MIT, Apache-2.0, etc. | Commercial | MIT, Apache-2.0, etc. | ✅ Obtain a commercial license from Slint. | ❗ Derivatives must use a license compatible with the project's license. The Derivate works can use Slint under any one of the licenses - (1) GPLv3, or (2) Commercial.| ❌ Slint source code may not be redistributed except under GPLv3.  |
| **Desktop, Mobile, Web** Application | GPL-compatible (e.g., MIT) | GPLv3 | GPLv3 | ✅ Include full source code (including build scripts), LICENSE file, GPL notice, and build/install instructions (GPL §6). <br>✅ Acknowledge Slint use in README or docs. | ❗ Derivatives must use a license compatible with the project's license. The Derivate works can use Slint under any one of the licenses - (1) Royalty-free, (2) GPLv3, or (3) Commercial. | ❌ Slint source code may not be redistributed except under GPLv3.  |
| **Embedded Systems** Application | GPL-compatible (e.g., MIT) | GPLv3 | GPLv3 | ✅ Include full source code (including build scripts), LICENSE file, GPL notice, and build/install instructions (GPL §6). <br>✅ Acknowledge Slint use in README or docs. | ❗ Derivatives must use a license compatible with the project's license. The Derivate works can use Slint under any one of the licenses - (1) GPLv3, or (2) Commercial. | ❌ Slint source code may not be redistributed except under GPLv3. |

#### Example Notice in README for Desktop, Mobile, Web Applications

```md
This project is licensed under the {your-project-license} License.

It uses the Slint GUI toolkit under the {selected-Slint-license} License. The Derivate works can use Slint under any one of the licenses - (1) [Royalty-free](https://slint.dev/terms-and-conditions#royalty-free), (2) [GPLv3](https://slint.dev/terms-and-conditions#gplv3), or (3) [Commercial](https://slint.dev/terms-and-conditions#license).

```

#### Example Notice in README for Embedded Systems

```md
This project is licensed under the {your-project-license} License.

It uses the Slint GUI toolkit under the {selected-Slint-license} License. The Derivate works can use Slint under any one of the licenses - (1) [Commercial](https://slint.dev/terms-and-conditions#license), or (2) [GPLv3](https://slint.dev/terms-and-conditions#gplv3).

```

### Royalty-free license

#### Who can use the Royalty-free license?

This license is suitable for those who develop desktop, mobile, or web applications and do not want to use open-source components under copyleft licenses.

#### What obligations do I need to fulfil to use the Royalty-free license?

You need to do one of the following:

1. Display the [`AboutSlint`](https://slint.dev/snapshots/master/docs/slint/src/language/widgets/aboutslint.html) widget in an "About" screen or dialog that is accessible from the top level menu of the Application. In the absence of such a screen or dialog, display the widget in the "Splash Screen" of the Application.

2. Display the [Slint attribution badge](https://github.com/slint-ui/slint/tree/master/logo/MadeWithSlint-logo-whitebg.png) on a public webpage, preferably where the binaries of your Application can be downloaded from, in such a way that it can be easily found by any visitor to that page.

#### Are there any limitations with the Royalty-free license?

1. You are not permitted to distribute or make Slint publicly available alone and without integration into an application. For this purpose you may use the Software under the GNU General Public License, version 3.

2. You are not permitted to use Slint within Embedded Systems. An Embedded System is a computer system designed to perform a specific task within a larger mechanical or electrical system.

3. You are not permitted to distribute an Application that exposes the APIs, in part or in total, of Slint.

4. You are not permitted to remove or alter any license notices (including copyright notices, disclaimers of warranty, or limitations of liability) contained within the source code form of Slint.

#### Scenario: What happens if my application is open-source (e.g. under MIT), forked by a different person and then redistributed?

The license does not restrict users on how they license their application. In the above scenario, the user may choose to use MIT-license for their application, which can be forked by a different person and then redistributed. If the forked application also uses Slint, then the person forking the application can choose to use Slint under any one of the licenses - Royalty-free, GPLv3, or paid license.

#### How are modifications to Slint itself covered under this license?

The license does not restrict 'if' and 'how' the modifications to Slint should be distributed. Say for example, Alice uses Slint under this new license to develop application A and modifies Slint in some way. She may choose to release the modifications to Slint under any license of her choice including any of the open source licenses. Alternatively she may decide not to release the modifications.

#### If Slint were to be taken over by a larger company or the current owners were to have a change of heart, can they revoke existing licenses?

We have a commitment to the larger Slint community to provide Slint under a Royalty-free license. This commitment is included in the [Contributors License Agreement (CLA)](http://cla-assistant.io/slint-ui/slint).

### GPLv3

#### If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?

No. You can license your program under any license compatible with the GPLv3 such as [https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses](https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses).

Refer to GPL FAQ [https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL](https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL).

#### My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?

Yes, provided the person distributing the proprietary program acquired a Slint proprietary license, such as the Slint Royalty-free license or a paid license, instead of using Slint under GPLv3. The other option would be to remove the dependency to Slint altogether.

#### My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?

You can add a note as part of your license that to distribute a proprietary licensed program, one can acquire a Slint proprietary license or the dependency to Slint should be removed.

#### My MIT-licensed program links to Slint GPLv3. Under what license can I release the entire work i.e my Program combined with Slint?

While your software modules can remain under the MIT-license, the work as a whole must be licensed under the GPL.

#### Scenario: Alice is a software developer, she wants her code to be licensed under MIT. She is developing an application "AliceApp" that links to Slint GPLv3. Alice also wants to allow that Bob, a user of AliceApp, can fork AliceApp into a proprietary application called BobApp

- Can Alice use the MIT license header to the source code of AliceApp application?

Yes. Alice can license her copyrighted source code under any license compatible with GPLv3. Refer FAQ [If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?](#if-i-link-my-program-with-slint-gplv3-does-it-mean-that-i-have-to-license-my-program-under-the-gplv3-too)

- Under what license should she distribute the AliceApp binary?

Under GPLv3. While the different software modules can remain under any license compatible with GPLv3, the work as a whole must be licensed under the GPL. Refer FAQ [My MIT-licensed program links to Slint GPLv3. Under what license can I release the binary of my program?](#my-mit-licensed-program-links-to-slint-gplv3-under-what-license-can-i-release-the-binary-of-my-program)

- How can Alice make it clear to Bob that he can distribute BobApp under a proprietary license?

Alice can add a note that Bob can distribute BobApp under a proprietary license if he either acquires a Slint proprietary license or removes the dependency to Slint.

### Paid License

#### What are the paid license options?

Check out the pricing plans on our website <https://slint.dev/pricing>.

### ✉️ Questions?

If you're unsure about licenses, reach out to the [Slint team](https://slint.dev/contact/) or consult with an open source licensing expert.

## Miscellaneous

### Do you provide Support?

Yes, check out our support options on our website <https://slint.dev/pricing#support>.
