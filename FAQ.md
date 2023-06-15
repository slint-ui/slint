
# Frequently Asked Questions: <!-- omit in toc -->

- [General](#general)
  - [Why does Slint use a domain specific langauage?](#why-does-slint-use-a-domain-specific-langauage)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [Royalty-free license](#royalty-free-license)
    - [Who can use the Royalty-free license?](#who-can-use-the-royalty-free-license)
    - [What obligations do I need to fulfil to use the Royalty-free license?](#what-obligations-do-i-need-to-fulfil-to-use-the-royalty-free-license)
    - [Are there any limitations with the Royalty-free license?](#are-there-any-limitations-with-the-royalty-free-license)
    - [Scenario: What happens if my application is open-source (e.g. under MIT), forked by a different person and then redistributed?](#scenario-what-happens-if-my-application-is-open-source-eg-under-mit-forked-by-a-different-person-and-then-redistributed)
    - [How are modifications to Slint itself covered under this licence?](#how-are-modifications-to-slint-itself-covered-under-this-licence)
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
- [Miscellaneous](#miscellaneous)
  - [Do you provide Support?](#do-you-provide-support)

## General

### Why does Slint use a domain specific langauage?

From our long experience of building UI toolkits, we have learnt that a domain
specific, declarative language is best suited to describe UIs. The Slint language
is easy and intuitive to use while being strict enough for our tools to analyze
and optimize to provide high graphics performance. Strictly typed binding
expressions offer a powerful and robust way for humans to declare relationships
between properties, even in complex user interfaces.

### Will there be API bindings to integrate with my favorite programming language?

We want to make it possible to use Slint with any programming language. We do
not favor one programming language over another. We have chosen to start with
three languages:

- Rust, our implementation language.
- C++, another systems programming language we have a lot of experience with.
- JavaScript, a popular dynamically typed language.

This choice builds the foundation that allows us to create bindings for most
types of programming languages.

## Licensing

Slint is available under a [royalty-free license](LICENSES/LicenseRef-Slint-Royalty-free-1.0.md) or [GNU GPLv3](LICENSES/GPL-3.0-only.txt), at your choice. For paid plans, visit our website [https://slint-ui.com](https://slint-ui.com) or contact us at [info@slint-ui.com](mailto:info@slint-ui.com).

### Royalty-free license

#### Who can use the Royalty-free license?

This license is suitable for those who develop desktop or web applications and do not want to use open-source components under copyleft licenses.

#### What obligations do I need to fulfil to use the Royalty-free license?

You need to do all of the following:

1. Display the [`AboutSlint`](https://slint-ui.com/snapshots/master/docs/slint/src/builtins/widgets.html#aboutslint) widget in an "About" screen or dialog that is accessible from the top level menu of your application.
2. Display the [Slint attribution badge](https://github.com/slint-ui/slint/tree/master/logo/madewithslint) on a public webpage, where the binaries of your application can be downloaded from, in such a way that it can be easily found by any visitor to that page.
3. You may not remove or alter any license notices (including copyright notices, disclaimers of warranty, or limitations of liability) contained within the source code form of Slint.
4. You allow us to use your application on our website and in advertising materials as a reference and to display your logo and trademark for this purpose.

#### Are there any limitations with the Royalty-free license?

There are 2 limitations:

1. You are not permitted to distribute or make Slint publicly available alone and without integration into an application. For this purpose you may use the Software under the GNU General Public License, version 3.
2. You are not permitted the use Slint within Embedded Systems. An Embedded System is a computer system designed to perform a specific task within a larger mechanical or electrical system. For the purposes of this license, mobile phones are not considered to be Embedded Systems.
  
#### Scenario: What happens if my application is open-source (e.g. under MIT), forked by a different person and then redistributed?

The license does not restrict users on how they license their application. In the above scenario, the user may choose to use MIT-license for their application, which can be forked by a different person and then redistributed. If the forked application also uses Slint, then the person forking the application can also use the Royalty-free license as long as the obligations are fulfilled. Alternatively, the person may use one of the other Slint license options such as commercial or GPLv3.

#### How are modifications to Slint itself covered under this licence?

The license does not restrict 'if' and 'how' the modifications to Slint should be distributed. Say for example, Alice uses Slint under this new license to develop application A and modifies Slint in some way. She may choose to release the modifications to Slint under any license of her choice including any of the open source licenses. Alternatively she may decide not to release the modifications.

#### If Slint were to be taken over by a larger company or the current owners were to have a change of heart, can they revoke existing licenses?

The commitment to provide Slint under a Royalty-free license has been included in the [Contributors License Agreement (CLA)](http://cla-assistant.io/slint-ui/slint) and thereby with the larger Slint community.

### GPLv3

#### If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?

No. You can license your program under any license compatible with the GPLv3 such as [https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses](https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses).

Refer to GPL FAQ [https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL](https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL).

#### My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?

Yes, provided the person distributing the proprietary program acquired a Slint proprietary license, such as the Slint Royalty-free license or a paid license, instead of using Slint under GPLv3. The other option would be to remove the dependency to Slint altogether.

#### My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?

You can add a note as part of your license that to distribute a proprietary licensed program, one can acquire a Slint proprietary license or the dependency to Slint should be removed.

#### My MIT-licensed program links to Slint GPLv3. Under what license can I release the binary of my program?

While your software modules can remain under the MIT-license, the work as a whole must be licensed under the GPL.
Hence, the binary must be made available under the GPLv3.

#### Scenario: Alice is a software developer, she wants her code to be licensed under MIT. She is developing an application "AliceApp" that links to Slint GPLv3. Alice also wants to allow that Bob, a user of AliceApp, can fork AliceApp into a proprietary application called BobApp

##### Can Alice use the MIT license header to the source code of AliceApp application?

Yes. Alice can license her copyrighted source code under any license compatible with GPLv3. Refer FAQ [If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?](#if-i-link-my-program-with-slint-gplv3-does-it-mean-that-i-have-to-license-my-program-under-the-gplv3-too)

##### Under what license should she distribute the AliceApp binary?

Under GPLv3. While the different software modules can remain under any license compatible with GPLv3, the work as a whole must be licensed under the GPL. Refer FAQ [My MIT-licensed program links to Slint GPLv3. Under what license can I release the binary of my program?](#my-mit-licensed-program-links-to-slint-gplv3-under-what-license-can-i-release-the-binary-of-my-program)

##### How can Alice make it clear to Bob that he can distribute BobApp under a proprietary license?

Alice can add a note that Bob can distribute BobApp under a proprietary license if he either acquires a Slint proprietary license or removes the dependency to Slint.

## Miscelleneous  

### Do you provide Support?

Yes, we offer paid support in addition to the community support on [GitHub Discussions](https://github.com/slint-ui/slint/discussions) and on our [Mattermost chat](https://chat.slint-ui.com). Please contact us at [info@slint-ui.com](mailto:info@slint-ui.com) to know more.
