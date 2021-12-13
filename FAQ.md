# Frequently Asked Questions:  <!-- omit in toc -->

- [General](#general)
  - [Where does the name come from?](#where-does-the-name-come-from)
  - [Is that name not going to be obsolete soon?](#is-that-name-not-going-to-be-obsolete-soon)
  - [Why are you creating a new markup language?](#why-are-you-creating-a-new-markup-language)
  - [Will there be API bindings to integrate with my favorite programming language?](#will-there-be-api-bindings-to-integrate-with-my-favorite-programming-language)
- [Licensing](#licensing)
  - [What is the commercial licensing model?](#what-is-the-commercial-licensing-model)
  - [Can I try SixtyFPS using the GPL version and then switch to the commercial license later?](#can-i-try-sixtyfps-using-the-gpl-version-and-then-switch-to-the-commercial-license-later)
  - [Is there a discount or reduction for independent developers or small businesses?](#is-there-a-discount-or-reduction-for-independent-developers-or-small-businesses)
  - [I want to develop a free software product using SixtyFPS, but I don't like the GPL and prefer to use a non-copyleft license.](#i-want-to-develop-a-free-software-product-using-sixtyfps-but-i-dont-like-the-gpl-and-prefer-to-use-a-non-copyleft-license)

# General

## Where does the name come from?

The name SixtyFPS comes from *60 frames per second*, which is the common refresh rate of screens.
This means that the screen shows a new image 60 times per second. To the human eye, animations updating
at this frequency look pleasantly fluid. Consequently, in games and user interfaces, 60 FPS is synonymous
with a perfectly smooth user interface.

## Is that name not going to be obsolete soon?

While newer screens are able to provide higher refresh rates, 60 is still a well accepted threshold
that has this historical value.

## Why are you creating a new markup language?

We are creating a markup language which is both editable by humans and machines. We hope it is possible
to pick up and understand, and at the same time strict enough for our tools to analyze and optimize
to provide a smooth interface on the screen. In our experience, a domain specific, declarative language
suits this purpose best. Strictly typed binding expressions offer a powerful and robust way for humans
to declare relationships between properties, even in complex user interfaces.

## Will there be API bindings to integrate with my favorite programming language?

We want to make it possible to use SixtyFPS with any programming language. We do not favor one programming
language over another. We have chosen to start with three languages:

  * Rust, our implementation language.
  * C++, another systems programming language we have a lot of experience with.
  * JavaScript, a popular dynamically typed language.

This choice builds the foundation that allows us to create bindings for most types of programming
languages.

# Licensing

SixtyFPS is available under two licenses:

 * GPLv3, for the growing ecosystem of Free and Open Source Software.
 * Commercial, for use in closed-source projects. See <https://sixtyfps.io/#offering>

## What is the commercial licensing model?

The basic principle behind our commercial licensing is that you start for free and pay when you're shipping.

We offer a perpetual license option and we generally deploy a per-product license, regardless of how many developers, designers, Q&A engineers are using SixtyFPS.

If this doesn't fit you, don't hesitate to contact us and we'd be happy to work together to find a solution.

You can find a more detailed overview of our commercial licensing and the pricing at <https://sixtyfps.io/pricing.html>.

## Can I try SixtyFPS using the GPL version and then switch to the commercial license later?

Yes. The GPL is a distribution license that applies only when you ship your application. You can
evaluate SixtyFPS and develop your product internally using the GPL license, and only acquire a commercial
license when you want to ship your product. If you choose a per seat licensing model, the time spent
developing needs to be accounted for. However, support for bug fixes requires a commercial license.

## Is there a discount or reduction for independent developers or small businesses?

Yes, check out our [Ambassador program](https://sixtyfps.io/ambassador-program.html)

## I want to develop a free software product using SixtyFPS, but I don't like the GPL and prefer to use a non-copyleft license.

You can still publish your own source code under a permissive license compatible with the GPL, such as BSD, MIT, or Apache license.
The distribution of a binary or a package containing SixtyFPS still needs to be licensed under the GPL.
It is up to those who want to distribute a non-free version of the application to acquire a commercial license.
