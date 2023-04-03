# The Slint Language

Slint comes with a easy to learn and use language to describe user interfaces
with. We designed this language to be readable to both humans and machines.
Slint can thus have excellent tooling on one side, while also enabling
designers and developers to see exactly what happens by reading the code
the machine uses to provide the user interfaces with.

This Slint language is either interpreted at run-time or compiled to native
code, which gets built into your application together with the code in the same
programming language providing the business logic. The Slint compiler can
optimize the user interface and any resources it uses at compile time, so
that user interfaces written in Slint use few resources, with regards to
performance and storage.

The Slint language enforces a separation of user interface from business logic,
using interfaces you can define for your project. This enables a fearless
cooperation between design-focused team members and those concentrating on the programming
side of the project. The Slint
