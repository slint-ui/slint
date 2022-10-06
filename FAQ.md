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

# General

## Where does the name come from?

The name *Slint* is derived from our design goals: **S**calable, **L**ightweight, **I**ntuitive, and **N**ative **T**oolkit.

## Why are you creating a new markup language?

We are creating a markup language which is both editable by humans and machines. We hope it is possible
to pick up and understand, and at the same time strict enough for our tools to analyze and optimize
to provide a smooth interface on the screen. In our experience, a domain specific, declarative language
suits this purpose best. Strictly typed binding expressions offer a powerful and robust way for humans
to declare relationships between properties, even in complex user interfaces.

## Will there be API bindings to integrate with my favorite programming language?

We want to make it possible to use Slint with any programming language. We do not favor one programming
language over another. We have chosen to start with three languages:

  * Rust, our implementation language.
  * C++, another systems programming language we have a lot of experience with.
  * JavaScript, a popular dynamically typed language.

This choice builds the foundation that allows us to create bindings for most types of programming
languages.

# Licensing

Slint can be used under either a [commercial license](./LICENSES/LicenseRef-Slint-commercial.md) or [GNU GPLv3](./LICENSES/GPL-3.0-only.txt), at your choice.

The commercial license can be provided for free if you help us promote Slint: Check out our
[ambassador license](https://slint-ui.com/ambassador-program.html).

## What are the commercial licensing options?

We offer 3 options under the commercial license - Ambassador, Flex and Buyout. All 3 options include a non-exclusive, perpetual, irrevocable, non-transferable right to use Slint.

The Ambassador license is a free license that can be provided in lieu of helping us promote Slint.

The Flex license is a per User-Seat license (with updates included as long as the subscription is active) to develop apps with Slint.
Distribution of such apps requires additional fees.

The Buyout license is a volume-based buyout license that includes unlimited User-Seats (with updates included) to develop apps with Slint as well as distribute such apps up to the purchased volume.

## Can I try Slint using the GPL version and then switch to the commercial license later?

Yes. You can evaluate Slint using the GPL license, and acquire the commercial license after the evaluation, with the option to transfer the development work from GPL to commercial for free.

## Is there a discount or reduction for independent developers or small businesses?

Yes, check out our [Ambassador program](https://slint-ui.com/ambassador-program.html)

## I want to develop a free software product using Slint, but I don't like the GPL and prefer to use a non-copyleft license.

A couple of options could be:

  * publish your own source code under a permissive license compatible with the GPL, such as BSD, MIT, or Apache license. However, the binary or the package
    containing Slint needs to be licensed under GPL,
  * consider of one of our [commercial licensing options](#what-are-the-commercial-licensing-options).
