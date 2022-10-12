# Frequently Asked Questions:  <!-- omit in toc -->

- [General](#general)
  - [Where does the name come from?](#where-does-the-name-come-from)
  - [Why are you creating a new markup language?](#why-are-you-creating-a-new-markup-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [What is the commercial licensing model?](#what-is-the-commercial-licensing-model)
  - [Can I try Slint using the GPL version and then switch to the commercial license later?](#can-i-try-slint-using-the-gpl-version-and-then-switch-to-the-commercial-license-later)
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

You can use Slint under either a [commercial license](./LICENSES/LicenseRef-Slint-commercial.md)
or [GNU GPLv3](./LICENSES/GPL-3.0-only.txt).

The commercial license can be provided for free if you help us promote Slint: Check out our
[ambassador program](https://slint-ui.com/ambassador-program.html).

### What are the commercial licensing options?

We offer - Ambassador, Flex and Buyout commercial licensing options. All options
include a non-exclusive, perpetual, irrevocable, non-transferable right to use
Slint. Updates are included in the Ambassador and Buyout options. With the Flex
option, updates are included as long as the subscription is active.

The Ambassador option is a free license that can be provided in lieu of helping
us promote Slint.

With the Flex option, you can choose the number of User-Seats you would need to
develop your applications with Slint. Before distribution of such applications
on embedded devices, you can purchase the required amount of distributions. At
any point of time, you could also switch to the Buyout option.

The Buyout option includes unlimited User-Seats and a prebuy of distribitions of
your Slint based applications on embedded devices.

### Can I try Slint using the GPL version and then switch to the commercial license later?

Yes. You can evaluate Slint using the GPL license, and obtain the commercial
license after the evaluation, with the option of transferring the code
developed under the GPL to commercial for free.

### Is there a discount or reduction for independent developers or small businesses?

Yes, check out our [Ambassador program](https://slint-ui.com/ambassador-program.html)

### I want to develop a free software product using Slint, but I don't like the GPL and prefer to use a non-copyleft license

You can publish your own source code under a permissive license compatible with
the GPL, such as BSD, MIT, or Apache license. However, the binary or the package
containing Slint needs to be licensed under GPL.
