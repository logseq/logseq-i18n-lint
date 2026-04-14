(ns test.db.property)

(def ^:large-vars/data-var built-in-properties
  (ordered-map
    :logseq.property/status {:title "Status"
                             :schema {:type :default}}
    :block/alias {:title "Alias"
                  :schema {:type :page}}))
