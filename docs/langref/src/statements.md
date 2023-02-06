# Statements

Inside callback handlers may contain more complex statements:

Assignment:

```slint,ignore
clicked => { some-property = 42; }
```

Self-assignment with `+=` `-=` `*=` `/=`

```slint,ignore
clicked => { some-property += 42; }
```

Calling a callback

```slint,ignore
clicked => { root.some-callback(); }
```

Conditional statements

```slint,ignore
clicked => {
    if (condition) {
        foo = 42;
    } else if (other-condition) {
        bar = 28;
    } else {
        foo = 4;
    }
}
```

Empty expression

```slint,ignore
clicked => { }
// or
clicked => { ; }
```
