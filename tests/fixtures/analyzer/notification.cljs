;; Notification messages
;; EXPECT: notification

;; Should detect: hardcoded notification messages
(notification/show! "File saved successfully" :success)
(notification/show! "Failed to delete page" :error)
(notification/show! "Changes discarded" :warning)

;; Should NOT detect: translated
(notification/show! (t :file-saved) :success)
