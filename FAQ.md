# Frequently Asked Questions:  <!-- omit in toc -->

- [General](#general)
  - [Where does the name come from?](#where-does-the-name-come-from)
  - [Why are you creating a new markup language?](#why-are-you-creating-a-new-markup-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [Can I license my code under a more permissive license than GPL?](#can-i-license-my-code-under-a-more-permissive-license-than-gpl)
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

### Can I license my code under a more permissive license than GPL?

Yes. You can publish your own source code under a permissive license compatible with
the GPL, such as BSD, MIT, or Apache license. However, the binary or the package
containing Slint needs to be licensed under GPL. For more info, you can check the following GPL FAQs:

- [https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL](https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL)
- [https://www.gnu.org/licenses/gpl-faq.en.html#IfLibraryIsGPL](https://www.gnu.org/licenses/gpl-faq.en.html#IfLibraryIsGPL)

### What are the different proprietary licensing options?

|   | Ambassador  | Flex  | Buyout  |
|--- |:---: |:---: |:---: |
| Price  | Free of charge  | from EUR 59 /user/month  | from EUR 5900  |
| Type of license  | Perpetual  | Perpetual  | Perpetual  |
| Users  | Unlimited  | As per license  | Unlimited  |
| Distribution on Desktop  | Unlimited  | Unlimited  | Unlimited  |
| Distribution on Embedded  | Unlimited  | As per purchased volume  | As per purchased volume  |
| Support  | Standard  | Standard  | Standard + 30 hours of Premium Support  |
| Updates  | Included  | Included  | Included  |
| Additional Obligations  | (1) consent to showcase the application,  (2) attribution of Slint and  (3) feedback to improve Slint.  | None  | None  |

The terms and conditions of the proprietary license is available [here](LICENSES/LicenseRef-Slint-commercial.md).

### What does perpetual mean?

The perpetual right allows you to use the version(s) of Slint, provided under the
proprietary license, for ever.

### What are the different support options?

|   | Standard Support  | Premium Support  |
|--- |:---: |:---: |
| Price  | Included with the proprietary license  | EUR 1650 per block of 10 hours  |
| Support Issues  | Tracked in public issue tracker  | Tracked in private issue tracker  |
| Support Scope  | Bug Fixes and defect corrections  | Answer technical questions,  conduct architecture reviews,  provide code snippets,  conduct trainings etc.  |
| Target Platforms  | Limited to official supported target platforms  | Support can be provided on  any platform and/or  operating system of choice  |

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
