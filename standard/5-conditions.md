# 5. Conditions

## 5.1 `bool`

1. True and false are the type `bool`, with the values `true` and `false` (D60).
2. **A condition must be a `bool`.** Numbers, text and lists never count as true or false: `if $count:` is refused;
   write `if $count > 0:` (D60).
   *Tests:* `no_truthiness`.

## 5.2 `if`

1. `if …:` runs its block when the condition is `true`. Further branches are `else if …:` and `else:`, and the whole
   chain closes with **one** `end if` (D71).
2. Exactly one branch runs, the first whose condition is `true`, or `else` when none is.

```
if $celsius > 30:
    std::print("hot", newline);
else if $celsius > 20:
    std::print("warm", newline);
else:
    std::print("cold", newline);
end if
```

*Tests:* `if_chain`.

## 5.3 `match`

Choosing between a tagged union's cases is `match` … `case` … `end match`; every case must be covered and branches
never fall through (D72). See section 9.3.
