# Frequently Asked Questions:  <!-- omit in toc -->

- [General](#general)
  - [Where does the name come from?](#where-does-the-name-come-from)
  - [Why are you creating a new markup language?](#why-are-you-creating-a-new-markup-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?](#if-i-link-my-program-with-slint-gplv3-does-it-mean-that-i-have-to-license-my-program-under-the-gplv3-too)
  - [My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?](#my-mit-licensed-program-links-to-slint-gplv3-can-someone-fork-my-program-to-build-and-distribute-a-proprietary-program)
  - [My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?](#my-mit-licensed-program-links-to-slint-gplv3-how-can-i-convey-to-someone-that-they-can-distribute-my-program-as-part-of-a-proprietary-licensed-program)
  - [My MIT-licensed program links to Slint GPLv3. Under what license can I release the binary of my program?](#my-mit-licensed-program-links-to-slint-gplv3-under-what-license-can-i-release-the-binary-of-my-program)
  - [What are the different proprietary licensing options?](#what-are-the-different-proprietary-licensing-options)
  - [What does perpetual mean?](#what-does-perpetual-mean)
  - [What are the different support options?](#what-are-the-different-support-options)
  - [Ambassador License](#ambassador-license)
    - [Why is the Ambassador license free-of-charge?](#why-is-the-ambassador-license-free-of-charge)
    - [When does the Ambassador license run out](#when-does-the-ambassador-license-run-out)
    - [For how long do you plan to offer the free-of-charge Ambasssador license?](#for-how-long-do-you-plan-to-offer-the-free-of-charge-ambasssador-license)
    - [How can I get the Ambassador license?](#how-can-i-get-the-ambassador-license)
    - [Do all contributors to my code have to sign up for the Ambassador license?](#do-all-contributors-to-my-code-have-to-sign-up-for-the-ambassador-license)
  - [Distributions](#distributions)
    - [Do I need to pay to distribute my application on desktop?](#do-i-need-to-pay-to-distribute-my-application-on-desktop)
    - [Do I need to pay to distribute my application on embedded devices?](#do-i-need-to-pay-to-distribute-my-application-on-embedded-devices)
    - [How much do I need to pay to distribute my application on embedded devices?](#how-much-do-i-need-to-pay-to-distribute-my-application-on-embedded-devices)
    - [What is the minimum distribution quantity that I can purchase?](#what-is-the-minimum-distribution-quantity-that-i-can-purchase)
  - [Miscelleneous](#miscelleneous)
    - [Is there a discount for independent developers or small businesses?](#is-there-a-discount-for-independent-developers-or-small-businesses)

## General

### Where does the name come from?

The name *Slint* is derived from our design goals: **S**calable, **L**ightweight,
**I**ntuitive, and **N**ative **T**oolkit.

### Why are you creating a new markup language?

We are creating a markup language which is both editable by humans and machines.
We hope it is possible to pick up and understand, and at the same time strict
enough for our tools to analyze and optimize to provide a smooth interface on
the screen. In our experience, a domain specific, declarative language suits
this purpose best. Strictly typed binding expressions offer a powerful and
robust way for humans to declare relationships between properties, even in
complex user interfaces.

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

Slint is available under either a [proprietary license](LICENSES/LicenseRef-Slint-commercial.md)
or [GNU GPLv3](LICENSES/GPL-3.0-only.txt)

### If I link my program with Slint GPLv3, does it mean that I have to license my program under the GPLv3, too?

No. You can license your program under any license compatible with the GPLv3 such
as [https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses](https://www.gnu.org/licenses/license-list.en.html#GPLCompatibleLicenses).
Refer to GPL FAQ [https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL](https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL).

### My MIT-licensed program links to Slint GPLv3. Can someone fork my program to build and distribute a proprietary program?

Yes, provided the person distributing the proprietary program acquired a Slint proprietary license instead of using Slint under GPLv3, or removed the dependency to Slint altogether.

### My MIT-licensed program links to Slint GPLv3. How can I convey to someone that they can distribute my program as part of a proprietary licensed program?

You can add a note as part of your license that to distribute a proprietary licensed program, one can get a Slint proprietary license or the dependency to Slint should be removed.

### My MIT-licensed program links to Slint GPLv3. Under what license can I release the binary of my program?

While your software modules can remain under the MIT-license, the work as a whole must be licensed under the GPL. 
Hence, the binary must be made available under the GPLv3.

### What are the different proprietary licensing options?

Our licensing options are available [here](https://slint-ui.com/#offering).

The terms and conditions of the proprietary license is available [here](LICENSES/LicenseRef-Slint-commercial.md).

### What does perpetual mean?

The perpetual right allows you to use the version(s) of Slint, provided under the
proprietary license, for ever.

### What are the different support options?

Standard support and Premium support.

The terms and conditions of standard support is available [here](https://slint-ui.com/support/slint_support_service_agreement) and premium support is available [here](https://slint-ui.com/support/slint_premium_support_service_agreement).

### Ambassador license

#### Why is the Ambassador license free-of-charge?

The license is provided free-of-charge to achieve the following goals:

a. accelerate adoption of Slint,
b. allow developers to use Slint under a non-GPL license,
c. create a strong feedback loop to improve Slint.

#### When does the Ambassador license run out?

The license grant is [perpetual](#what-does-perpetual-mean), which means that you can use Slint free-of-charge forever. The perpetual nature of the license also implies that even if we need to modify the terms of the license in the future, the modified terms will not apply to already granted licenses.

#### For how long do you plan to offer the free-of-charge Ambasssador license?

Forever. However, we may modify the terms of the license in the future based on user feedback and business needs.

#### How can I get the Ambassador license?

The license is automatically granted on signing the license agreement [here](https://slint-ui.com/ambassador-program.html#application). The authorized signatory could be the code owner, primary maintainer, or in case of an organisation, the relevant person authorized to sign contracts on behalf of the organisation.

#### Do all contributors to my code have to sign up for the Ambassador license?

No. All contributions to the code is covered under the Ambassador license. Hence, contributors can use Slint under the same license within this scope.

### Distributions

#### Do I need to pay to distribute my application on desktop?

No. Our proprietary license includes unlimited distribution on desktop.

#### Do I need to pay to distribute my application on embedded devices?

For Flex and Buyout licenses, additional fees are applicable for distribution on embedded devices.
The Ambassador license includes unlimited distribution on embedded devices.

#### How much do I need to pay to distribute my application on embedded devices?

The fee depends upon the quantity of distribution purchased. The higher the quantity, the lower the per device fee.
As an example, 1000 distributions cost EUR 3500 (per device fee of EUR 3.50) while 5000 distributions cost EUR 15400 (per device fee of EUR 3.08).
Please [contact us](https://slint-ui.com/staging/#contact_us) if you are interested to know more.

#### What is the minimum distribution quantity that I can purchase?

The minimum quantity is 1.
You can purchase in quantities of 1, 10, 100, 500, 1000, 3000 and 5000.
Please [contact us](https://slint-ui.com/staging/#contact_us) if you are interested in higher volumes.

### Miscelleneous

### Is there a discount for independent developers or small businesses?

The [Ambassador license](#ambassador-license) is a free-of-charge license suitable for independent developers or small businesses.
