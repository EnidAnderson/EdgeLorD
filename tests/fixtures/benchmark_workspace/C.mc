;; File C.mc - Deterministic diagnostic: parse error with unicode
;; Contains:
;; - ZWJ sequence (family emoji)
;; - Combining mark (é as e + combining accent)
;; - Surrogate pair emoji (crab)
;; - Parse error: undefined_symbol (will generate deterministic diagnostic)

(def família👨‍👩‍👧‍👦 (hole café))

;; This intentional error creates a deterministic diagnostic:
;; undefined_symbol at the line above (spans the identifier)
;; Useful for cross-file invalidation testing: editing this file
;; should invalidate A's cache (via B's dependency)
