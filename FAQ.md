# Frequently Asked Questions:  <!-- omit in toc -->

- [General](#general)
  - [Where does the name come from?](#where-does-the-name-come-from)
  - [Why are you creating a new markup language?](#why-are-you-creating-a-new-markup-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [What are the commercial licensing options?](#what-are-the-commercial-licensing-options)
  - [What does perpetual mean?](#what-does-perpetual-mean)
  - [Are updates included?](#are-updates-included)
  - [Can I try Slint using the Ambassador option and then switch to a paid option later?](#can-i-try-slint-using-the-ambassador-option-and-then-switch-to-a-paid-option-later)
  - [Can I try Slint using the GPL version and then switch to the commercial license later?](#can-i-try-slint-under-the-gpl-and-then-switch-to-the-commercial-license-later)
  - [Is there a discount or reduction for independent developers or small businesses?](#is-there-a-discount-or-reduction-for-independent-developers-or-small-businesses)
  - [I want to develop a free software product using Slint, but I don't like the GPL and prefer to use a non-copyleft license.](#i-want-to-develop-a-free-software-product-using-slint-but-i-dont-like-the-gpl-and-prefer-to-use-a-non-copyleft-license)

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

You are free to use Slint under either [GNU GPLv3](./LICENSES/GPL-3.0-only.txt) or 
a [commercial license](./LICENSES/LicenseRef-Slint-commercial.md). We also offer a completely 
free-of-charge commercial license under the [ambassador program](https://slint-ui.com/ambassador-program.html).

### What are the commercial licensing options?

We offer - Ambassador, Flex and Buyout commercial licensing options. All three 
options include a perpetual right to use Slint to develop applications, unlimited 
distribution of such applications on desktops and [standard support](https://slint-ui.com/support/slint_support_service_agreement).

- The Ambassador option is a free license that can be provided in lieu of helping
us promote Slint. Apart from the standard inclusiosn, unlimited User-Seats, updates and unlimited distribution 
of Slint based applications on embedded devices are included.

- With the Flex option, you can purchase the required number of User-Seats needed to
develop your applications with Slint. In addition to the standard inclusions, updates are included as long 
as the subscription is active.

- The Buyout option includes unlimited User-Seats, updates and 30 hours of [premium support](https://slint-ui.com/support/slint_premium_support_service_agreement) in addition to the standard inclusions. 
Slint based applications can be distributed on embedded devices as per purchased distribution license pack.

### Is the Ambassador really for free? What is the catch?

The Ambassador is really for free! The only 'catch' is to help us promote Slint in the following ways:

- Showcase: You grant us the right to use your name, logo and your Products in our marketing materials,

- Spread the word: You will include reasonably visible notices in the documentation, marketing materials 
and social media channels related to your Products that such were built with Slint,

- Give feedback: You agree to provide relevant feedback that would help us improve Slint. 
We may use any such feedback in testimonials.

### What does perpetual mean?

The perpetual right allows you to use the version(s) of Slint, provided under the
commercial license, for ever.

### Are updates included?

Yes, updates are included with Ambassador and Buyout. With Flex, updates are included as long as
the subscription is active.

### What are the different support options that you offer?

We offer [standard support](https://slint-ui.com/support/slint_support_service_agreement) and [premium support](https://slint-ui.com/support/slint_premium_support_service_agreement). 

Standard support is included with the commercial license.

Premium support is offered to both open source users and commercial customers. Support can be purchased in blocks of 10 hours
for EUR 1650. Please [contact us](https://slint-ui.com/staging/#contact_us) if you are interested in purchasing premium support.

### Do I need to buy distribution licenses if I distribute my Slint based application on desktop?

No. Our commercial license includes unlimited distribution of Slint based application on desktop.

### Do I need to buy distribution licenses if I distribute my Slint based application on embedded devices?

Depends on the license option. The Ambassador includes unlimited distribution of Slint based application 
on embedded devices. The Buyout includes such distribution limited to the quantity of purchased distribution
license pack. With Flex, you would need to buy the appropriate distribution license pack to cover the required 
quantity of distributions.

### How much does the distribution license on embedded devices cost?

Distribution licenses are sold in volume packs. The bigger the pack, the cheaper the cost of a single distribution license.
For example, a pack of 1000 distributions cost EUR 3500 while a pack of 5000 distributions cost EUR 15400. Please [contact us](https://slint-ui.com/staging/#contact_us) if you are interested to know more.

### What is the minimum distribution quantity that I can purchase?

The minimum distribution quantity is 1.

Other distribution packs sizes are 10, 100, 500, 1000, 3000 and 5000. 
Please [contact us](https://slint-ui.com/staging/#contact_us) if you are interested in a bigger distribution pack.

### Can I try Slint using the Ambassador option and then switch to a paid option later?

Yes. You can start using Slint with the Ambassador option, and switch to a paid option later. 
However we retain the marketing rights obtained under the Ambassador Program for existing materials.

### Can I try Slint under the GPL and then switch to the commercial license later?

Yes. You can evaluate Slint using the GPL license, and obtain the commercial license after the evaluation, 
with the option of transferring the code developed under the GPL to commercial in order to avoid any [copy-left obligations](https://www.gnu.org/licenses/copyleft.en.html).

### Is there a discount or reduction for independent developers or small businesses?

Yes, check out our [ambassador program](https://slint-ui.com/ambassador-program.html)

### I want to develop a free software product using Slint, but I don't like the GPL and prefer to use a non-copyleft license

You can publish your own source code under a permissive license compatible with
the GPL, such as BSD, MIT, or Apache license. However, the binary or the package
containing Slint needs to be licensed under GPL.
