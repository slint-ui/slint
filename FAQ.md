
# Frequently Asked Questions: <!-- omit in toc -->

- [General](#general)
  - [Why does Slint use a domain specific language?](#why-does-slint-use-a-domain-specific-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [Can I keep my own code under a permissive license such as MIT?](#can-i-keep-my-own-code-under-a-permissive-license-such-as-mit)
  - [Royalty-free license](#royalty-free-license)
    - [Who can use the Royalty-free license?](#who-can-use-the-royalty-free-license)
    - [What obligations do I need to fulfil to use the Royalty-free license?](#what-obligations-do-i-need-to-fulfil-to-use-the-royalty-free-license)
    - [Are there any limitations with the Royalty-free license?](#are-there-any-limitations-with-the-royalty-free-license)
    - [What counts as an embedded system?](#what-counts-as-an-embedded-system)
    - [Scenario: What happens if my application is open-source (e.g. under MIT), forked by a different person and then redistributed?](#scenario-what-happens-if-my-application-is-open-source-eg-under-mit-forked-by-a-different-person-and-then-redistributed)
    - [How are modifications to Slint itself covered under this license?](#how-are-modifications-to-slint-itself-covered-under-this-license)
    - [If Slint were to be taken over by a larger company or the current owners were to have a change of heart, can they revoke existing licenses?](#if-slint-were-to-be-taken-over-by-a-larger-company-or-the-current-owners-were-to-have-a-change-of-heart-can-they-revoke-existing-licenses)
  - [GPLv3](#gplv3)
    - [If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?](#if-i-link-my-program-with-slint-gplv3-does-it-mean-that-i-have-to-license-my-program-under-the-gplv3-too)
    - [My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?](#my-mit-licensed-program-links-to-slint-gplv3-can-someone-fork-my-program-to-build-and-distribute-a-proprietary-program)
    - [My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?](#my-mit-licensed-program-links-to-slint-gplv3-how-can-i-convey-to-someone-that-they-can-distribute-my-program-as-part-of-a-proprietary-licensed-program)
    - [My MIT-licensed program links to Slint GPLv3. Under what license can I release the entire work i.e my Program combined with Slint?](#my-mit-licensed-program-links-to-slint-gplv3-under-what-license-can-i-release-the-entire-work-ie-my-program-combined-with-slint)
  - [Commercial License](#commercial-license)
    - [What are the Commercial license options?](#what-are-the-commercial-license-options)
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

Read more in our blog post on [declarative versus imperative UI](https://slint.dev/blog/domain-specific-language-vs-imperative-for-ui).

### Will there be API bindings to integrate with my favorite programming language?

We want to make it possible to use Slint with any programming language, and we
don't favor one over another. Slint currently supports four:

- Rust, our implementation language.
- C++, another systems programming language we have a lot of experience with.
- JavaScript, a popular dynamically typed language.
- Python, widely used for tooling, scripting, and data science.

This choice builds the foundation that allows us to create bindings for most
types of programming languages.

## Licensing

You can use Slint under ***any*** of the following licenses, at your choice:

1. [Royalty-free license](LICENSES/LicenseRef-Slint-Royalty-free-2.0.md),
2. [GNU GPLv3](LICENSES/GPL-3.0-only.txt),
3. [Commercial license](LICENSES/LicenseRef-Slint-Software-3.0.md).

### Can I keep my own code under a permissive license such as MIT?

Yes — whichever Slint license you use, your own source files can stay under a permissive license such as MIT or Apache-2.0.

Slint itself stays under its three licenses (GPLv3, Royalty-free, or Commercial), so the combined product you distribute is licensed one of two ways:

- **Open-source the whole product? Use the GPLv3.**
  The product as a whole is then GPL: free, on any platform including embedded.
  Your own files keep their permissive headers (MIT is GPL-compatible).
- **Want your own terms instead? Use the Royalty-free or Commercial license.**
  The Royalty-free license is free for desktop, mobile, and web, as long as you show the Slint attribution.
  The Commercial license covers embedded and lets you set your own terms.

For detailed scenarios, see the [GPLv3 questions below](#gplv3).

### Royalty-free license

#### Who can use the Royalty-free license?

This license is suitable for those who develop desktop, mobile, or web applications and do not want to use open-source components under copyleft licenses.

#### What obligations do I need to fulfil to use the Royalty-free license?

You need to do one of the following:

1. Display the [`AboutSlint`](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/misc/aboutslint/) widget in an "About" screen or dialog that is accessible from the top level menu of the Application. In the absence of such a screen or dialog, display the widget in the "Splash Screen" of the Application.

2. Display the [Slint attribution badge](https://github.com/slint-ui/slint/tree/master/logo/MadeWithSlint-logo-whitebg.png) on a public webpage, preferably where the binaries of your Application can be downloaded from, in such a way that it can be easily found by any visitor to that page.

#### Are there any limitations with the Royalty-free license?

1. You are not permitted to distribute or make Slint publicly available alone and without integration into an application. For this purpose you may use the Software under the GNU General Public License, version 3.

2. You are not permitted to use Slint within Embedded Systems. An Embedded System is a computer system designed to perform a specific task within a larger mechanical or electrical system.

3. You are not permitted to distribute an Application that exposes the APIs, in part or in total, of Slint.

4. You are not permitted to remove or alter any license notices (including copyright notices, disclaimers of warranty, or limitations of liability) contained within the source code form of Slint.

#### What counts as an embedded system?

An **embedded system** is a computer system that performs a specific task within a larger mechanical or electrical system — for example the controller driving the screen of an appliance, a point-of-sale terminal, or a car dashboard. The Commercial license or the GPL are suitable licenses for Slint for such systems.

It's **not** an embedded system when your application runs on a user's own general-purpose computer or phone, installed as one application among many. That's the desktop, mobile, and web case the Royalty-free License covers for free.

#### Scenario: What happens if my application is open-source (e.g. under MIT), forked by a different person and then redistributed?

The license does not restrict users on how they license their application. In the above scenario, the user may choose to use MIT-license for their application, which can be forked by a different person and then redistributed. If the forked application also uses Slint, then the person forking the application can choose to use Slint under any one of the licenses - Royalty-free, GPLv3, or Commercial license.

#### How are modifications to Slint itself covered under this license?

The license does not restrict 'if' and 'how' the modifications to Slint should be distributed. Say for example, Alice uses Slint under this new license to develop application A and modifies Slint in some way. She may choose to release the modifications to Slint under any license of her choice including any of the open source licenses. Alternatively she may decide not to release the modifications.

#### If Slint were to be taken over by a larger company or the current owners were to have a change of heart, can they revoke existing licenses?

We have a commitment to the larger Slint community to provide Slint under a Royalty-free license.
This commitment is part of our [open-source pledge](CONTRIBUTING.md#our-open-source-pledge).

### GPLv3

#### If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?

No. You can license your program under any license compatible with the GPLv3 such as [https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses](https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses).

Refer to GPL FAQ [https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL](https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL).

#### My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?

Yes, provided the person distributing the proprietary program acquired a Slint proprietary license, such as the Slint Royalty-free license or a Commercial license, instead of using Slint under GPLv3. The other option would be to remove the dependency to Slint altogether.

#### My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?

You can add a note as part of your license that to distribute a proprietary licensed program, one can acquire a Slint proprietary license or the dependency to Slint should be removed.

#### My MIT-licensed program links to Slint GPLv3. Under what license can I release the entire work i.e my Program combined with Slint?

While your software modules can remain under the MIT-license, the work as a whole must be licensed under the GPL.

### Commercial License

#### What are the Commercial license options?

Check out the pricing plans on our website <https://slint.dev/pricing>.

## Miscellaneous

### Do you provide Support?

Yes, check out our support options on our website <https://slint.dev/pricing#support>.
