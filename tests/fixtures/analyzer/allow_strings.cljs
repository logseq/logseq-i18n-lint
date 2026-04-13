;; Allow strings test - these should NOT be reported

;; CSS class names (single word, lowercase with hyphens)
[:div {:class "text-sm"}]
[:div {:class "flex"}]
[:div {:class "px-2"}]
[:div {:class "rounded-lg"}]

;; URLs
[:a {:href "https://logseq.com"}]
[:a {:href "http://example.com/path"}]

;; File extensions
[:span ".pdf"]
[:span ".md"]

;; Color codes
[:div {:style {:color "#ff0000"}}]
[:div {:style {:bg "#eee"}}]

;; Empty and single char
[:div ""]
[:div " "]
[:div "/"]

;; All-caps constants
(def STATUS "TODO")
(def STATE "DONE")

;; Technical identifiers (not in hiccup context)
(get block "block/title")
(get page "page/name")

;; Brand name (in allow_strings list)
[:span "Logseq"]
