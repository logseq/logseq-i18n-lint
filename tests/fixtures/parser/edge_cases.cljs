;; Edge cases test fixtures

;; Multi-byte characters
(def greeting "你好世界")
(def emoji "Hello 🌍")

;; Very long line
(def long-str "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.")

;; Comma as whitespace (Clojure treats comma as whitespace)
{:a 1, :b 2, :c 3}

;; Symbols with special characters
foo-bar
foo_bar
foo.bar/baz
+
-
*
/

;; Namespaced keywords
:foo/bar
::auto
