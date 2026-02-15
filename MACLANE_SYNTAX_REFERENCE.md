# MacLane Syntax Reference

**Date**: 2026-02-08
**Purpose**: Quick reference for correct MacLane/Comrade Lisp syntax

## Quote Syntax

### ✅ CORRECT: Use Apostrophe `'`
MacLane uses **apostrophe for quoting**, not the word "quote":

```lisp
;; Quote a symbol
(def x 'hello)

;; Quote a list
(def y '(a b c))

;; Quote an s-expression
(def z '(touch foo))
```

### ❌ INCORRECT: `(quote ...)` is NOT valid
The word "quote" is just a regular identifier, not a special form:

```lisp
;; ERROR: invalid symbol 'quote'
(def x (quote hello))

;; ERROR: invalid symbol 'quote'
(def y (quote (a b c)))
```

**Why?** The lexer tokenizes `'` as `TokenKind::Quote`, which the parser handles specially. The word "quote" has no special meaning.

## Quote Variants

MacLane supports multiple quote forms:

```lisp
'x          ;; Quote (apostrophe)
`x          ;; Quasiquote (backtick)
,x          ;; Unquote (comma)
,@xs        ;; Unquote-splicing (comma-at)
```

## Touch-Before-Def Rule

**Critical**: You MUST `touch` a symbol before you can `def` it:

```lisp
;; ✅ CORRECT
(begin
  (touch my-value)
  (def my-value 'hello)
)

;; ❌ INCORRECT
(begin
  (def my-value 'hello)  ;; ERROR: my-value not touched
)
```

## Comment Syntax

MacLane uses **Common Lisp style comments** with semicolon:

```lisp
;; Full-line comment (convention: double semicolon)
(def x 'value)  ; End-of-line comment (single semicolon)

;;; Section header (convention: triple semicolon)
```

### NOT Valid
```
-- NOT a comment
// NOT a comment  
# NOT a comment
```

## Kernel Primitives

MacLane has only a few kernel primitives:

```lisp
(begin ...)              ;; Lexical scope
(touch symbol)           ;; Introduce a symbol (1 arg)
(def name value)         ;; Define a binding (2 args)
(rule lhs rhs cert)      ;; Define a rewrite rule (3 args)
(sugar name pattern tpl) ;; Define a macro (3 args)
```

Everything else is derived via macros.

## Common Errors

### 1. Using `(quote ...)` instead of `'`
```lisp
;; ❌ WRONG
(def x (quote hello))

;; ✅ RIGHT
(def x 'hello)
```

### 2. Def without Touch
```lisp
;; ❌ WRONG
(def my-value 'hello)

;; ✅ RIGHT
(touch my-value)
(def my-value 'hello)
```

### 3. Wrong Comment Syntax
```lisp
;; ❌ WRONG
-- This is not a comment

;; ✅ RIGHT
;; This is a comment
```

### 4. Invalid Arity
```lisp
;; ❌ WRONG
(touch x y)  ;; touch expects 1 arg

;; ✅ RIGHT
(touch x)
(touch y)
```

## Examples from Prelude

Real examples from `clean_kernel/madlib/prelude/prelude.maclane`:

```lisp
;; Touch then def
(touch D0)
(def D0
  (cons '(touch x)
        (cons '(def x 'v) nil)))

;; Rewrite rule with quoted cert
(rule (concat_doctrine nil D)
      D
      '(concat_doctrine nil-left-id))

;; Nested quotes
(def F0
  (cons '(def facet/inc '(classifier/trivial))
        nil))
```

## Lexer Token Reference

From `clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule/src/lexer.rs`:

- `'` → `TokenKind::Quote`
- `` ` `` → `TokenKind::QuasiQuote`
- `,` → `TokenKind::Unquote`
- `,@` → `TokenKind::UnquoteSplicing`
- `;` → Line comment (consumed by lexer)
- `(` → `TokenKind::LParen`
- `)` → `TokenKind::RParen`

## Related Documentation

- **Lexer**: `clean_kernel/satellites/src/surface_maclane/NewSurfaceSyntaxModule/src/lexer.rs`
- **Prelude**: `clean_kernel/madlib/prelude/prelude.maclane`
- **Guidance**: `clean_kernel/guidance_from_comrade_lisp.md`
- **Test file**: `EdgeLorD/test_examples/simple_error.maclane`
