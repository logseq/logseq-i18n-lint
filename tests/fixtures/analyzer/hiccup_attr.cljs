;; Hiccup keyword attributes
;; EXPECT: hiccup-attr

;; Should detect: UI attribute with hardcoded value
[:input {:placeholder "Search pages..."}]
[:img {:alt "User avatar"}]
[:div {:title "Click to expand"}]
[:button {:aria-label "Close dialog"}]

;; Should NOT detect: non-UI attributes
[:div {:class "text-sm flex"}]
[:div {:id "main-content"}]
[:div {:on-click handler}]
[:div {:style {:color "red"}}]

;; Should NOT detect: translated
[:input {:placeholder (t :search-placeholder)}]
