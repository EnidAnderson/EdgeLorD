;; File A.mc - Imports B
;; Head of the import chain: A -> B -> C

(import B)

(def main (hole result))

;; This file has no errors when B compiles cleanly.
;; When C.mc is edited:
;;   1. C's content_hash changes
;;   2. B recompiles (imports C)
;;   3. workspace_snapshot_hash changes
;;   4. A's Phase 1 cache should MISS with reason: deps_changed
;;   5. A should recompile (even though A's content unchanged)
;;
;; When A is touched (no-op edit):
;;   1. A's DV increments, content_hash unchanged
;;   2. A's Phase 1 cache should MISS with reason: deps_changed
;;   3. A should recompile (workspace state changed)
;;
;; When A is edited directly:
;;   1. A's content_hash changes
;;   2. A's Phase 1 cache should MISS with reason: content_changed
;;   3. A should recompile
