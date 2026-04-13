;; Mixed patterns integration test
;; This file combines multiple patterns for end-to-end testing

(ns myapp.components.editor
  (:require [frontend.ui :as ui]
            [frontend.modules.shortcut.config :as shortcut]
            [frontend.state :as state]
            [logseq.shui.ui :as shui]))

;; Pattern: hiccup-text - EXPECT: hiccup-text
(defn editor-toolbar []
  [:div.toolbar {:class "flex items-center"}
    [:span "Bold"]
    [:span "Italic"]
    (ui/button (t :undo))])

;; Pattern: hiccup-attr - EXPECT: hiccup-attr
(defn search-box []
  [:input {:placeholder "Search pages and blocks..."
           :class "w-full px-2"
           :on-change handle-change}])

;; Pattern: notification - EXPECT: notification
(defn save-handler []
  (if (save-success?)
    (notification/show! "Saved successfully" :success)
    (notification/show! (t :save-failed) :error)))

;; Pattern: fn-arg-text - EXPECT: fn-arg-text
(defn action-buttons []
  [:div
    (shui/button {:label "Delete page"})
    (shui/button {:label (t :confirm)})])

;; Pattern: conditional-text - EXPECT: conditional-text
(defn status-indicator [loading?]
  [:div
    (if loading?
      "Loading content..."
      "All loaded")])

;; Pattern: def - NOT reported: plain string in def is a data constant
(def error-fallback "Something went wrong")

;; Pattern: format-string - NOT reported: format is outside UI context here
(defn item-count [n]
  (goog.string/format "Found %d matching items" n))

;; Pattern: let-text - NOT reported: let binding is outside UI context (title is a Symbol in hiccup, not a String)
(defn render-page [page]
  (let [title "Untitled page"]
    [:h1 title]))

;; Should NOT report any of these:
(js/console.log "render called")
(log/debug "Component mounted" component-id)
[:div {:class "bg-white rounded-lg shadow"}]
[:a {:href "https://docs.logseq.com"}]
[:span "Logseq"]
(re-find #"blocks?" input)
