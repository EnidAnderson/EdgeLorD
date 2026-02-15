;; File B.mc - Imports C
;; Part of the import chain: A -> B -> C

(import C)

(def process_family (hole family_data))

;; When C.mc is edited, B's content hash changes, which should
;; invalidate A's cache (since A depends on B's exports)
