//! The only stylesheet of the admin pages (inlined in a single `<style>`
//! block; the CSP allows `style-src 'unsafe-inline'` and nothing else, so
//! there are no web fonts, images or scripts here).
//!
//! A minimal dark theme that follows the launcher's tokens
//! (`docs/design/ui-style.md`): the same surface tones, text colors, accent,
//! focus ring, 4/8 px spacing and 4/6/8 px radii, with the system font stack.
//! Filled accent buttons approve; danger buttons are tonal red with a red
//! outline, so the two never read alike. One column under 640 px.
//!
//! Hooks: `body.admin`, `header.admin-header`, `main.admin-main`,
//! `section.admin-section` (`#reports`, `#pending`, `#hidden`, `#bans`,
//! `#audit`), `article.admin-card` (+ `.report`, `.version`,
//! `.hidden-comment`), `table.admin-table`, `form.admin-form` (+ `.ban-by-id`),
//! `button` and `a.button` (`.primary`, `.danger`), `.muted`, `.badge`,
//! `.thumbs`, `.report-details`, and `.notice` / `.error` (message pages).

pub const ADMIN_CSS: &str = "\
:root{color-scheme:dark;\
--bg:#0f1115;--surface-1:#15181d;--surface-2:#1b1f25;--surface-3:#232830;--surface-4:#2c323c;\
--border:#262b33;--border-strong:#363c46;--text:#e7e9ec;--text-muted:#9aa2ad;\
--accent:#d23a1e;--accent-hover:#b52f17;--on-accent:#ffffff;--accent-text:#ff8a66;\
--focus-ring:#5aa9ff;--danger-text:#ff8f86;--danger-border:#8a3a36;--danger-bg:#2a1716;--danger-hover:#3a1d1b;\
--warning-border:#8a6a24;--info-border:#3b5688;\
--space-1:4px;--space-2:8px;--space-3:12px;--space-4:16px;--space-5:24px;--space-6:32px;\
--radius-sm:4px;--radius-md:6px;--radius-lg:8px;--control-height:32px;\
--easing:cubic-bezier(0.2,0,0,1);--duration:150ms}\
*,*::before,*::after{box-sizing:border-box}\
body.admin{margin:0;background:var(--bg);color:var(--text);\
font:14px/1.5 'Segoe UI Variable Text','Segoe UI',system-ui,sans-serif}\
h1,h2,h3{margin:0;font-weight:600;line-height:1.3}\
p{margin:0 0 var(--space-2)}\
code{font-family:ui-monospace,'Cascadia Mono',Consolas,monospace;font-size:12px;overflow-wrap:anywhere}\
a{color:var(--accent-text);text-underline-offset:2px}\
a:hover{color:var(--text)}\
:focus-visible{outline:2px solid var(--focus-ring);outline-offset:2px}\
header.admin-header{position:sticky;top:0;z-index:1;display:flex;flex-wrap:wrap;gap:var(--space-2) var(--space-4);\
justify-content:space-between;align-items:center;padding:var(--space-3) var(--space-5);\
background:var(--surface-1);border-bottom:1px solid var(--border)}\
header.admin-header h1{font-size:16px}\
header.admin-header div{display:flex;align-items:center;gap:var(--space-3);color:var(--text-muted)}\
header.admin-header form.admin-form{margin:0}\
main.admin-main{max-width:1100px;margin:0 auto;padding:var(--space-5)}\
section.admin-section{margin:0 0 var(--space-6)}\
section.admin-section h2{display:flex;align-items:center;gap:var(--space-2);margin-bottom:var(--space-3);\
padding-bottom:var(--space-2);font-size:18px;border-bottom:1px solid var(--border)}\
article.admin-card{margin:0 0 var(--space-3);padding:var(--space-3) var(--space-4);background:var(--surface-1);\
border:1px solid var(--border);border-left-width:3px;border-radius:var(--radius-lg)}\
article.admin-card.report{border-left-color:var(--warning-border)}\
article.admin-card.version{border-left-color:var(--info-border)}\
article.admin-card.hidden-comment{border-left-color:var(--border-strong)}\
article.admin-card h3{margin-bottom:var(--space-2);font-size:15px}\
article.admin-card pre{margin:var(--space-2) 0;padding:var(--space-2) var(--space-3);white-space:pre-wrap;\
overflow-wrap:anywhere;font:inherit;background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-md)}\
.report-details{margin:0 0 var(--space-2);padding-left:var(--space-5);color:var(--text-muted)}\
.report-details:empty{display:none}\
table.admin-table{width:100%;margin:var(--space-2) 0;border-collapse:collapse;font-size:13px}\
table.admin-table th,table.admin-table td{padding:var(--space-2) var(--space-3);text-align:left;vertical-align:top;\
border-bottom:1px solid var(--border);overflow-wrap:anywhere}\
table.admin-table th{color:var(--text-muted);font-weight:600}\
table.admin-table tbody tr:hover{background:var(--surface-2)}\
table.admin-table td form.admin-form{margin:0}\
form.admin-form{display:inline-flex;flex-wrap:wrap;gap:var(--space-2);align-items:center;\
margin:var(--space-2) var(--space-2) 0 0;vertical-align:top}\
form.admin-form.ban-by-id{display:flex;margin:var(--space-3) 0 0;padding:var(--space-3);\
background:var(--surface-1);border:1px solid var(--border);border-radius:var(--radius-lg)}\
input[type=text]{min-width:0;height:var(--control-height);padding:0 var(--space-3);color:var(--text);font:inherit;\
background:var(--bg);border:1px solid var(--border-strong);border-radius:var(--radius-md);\
transition:border-color var(--duration) var(--easing)}\
input[type=text]:hover{border-color:var(--text-muted)}\
input[type=text]::placeholder{color:var(--text-muted)}\
input[name=reason]{width:240px}\
.visually-hidden{position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}\
button,a.button{display:inline-flex;align-items:center;justify-content:center;height:var(--control-height);\
padding:0 var(--space-4);color:var(--text);font:inherit;font-weight:500;text-decoration:none;cursor:pointer;\
background:var(--surface-3);border:1px solid var(--border-strong);border-radius:var(--radius-md);\
transition:background-color var(--duration) var(--easing),border-color var(--duration) var(--easing),\
transform 100ms var(--easing)}\
button:hover,a.button:hover{color:var(--text);background:var(--surface-4)}\
button:active,a.button:active{transform:scale(0.98)}\
button.primary,a.button.primary{color:var(--on-accent);background:var(--accent);border-color:var(--accent)}\
button.primary:hover,a.button.primary:hover{color:var(--on-accent);background:var(--accent-hover);border-color:var(--accent-hover)}\
button.danger{color:var(--danger-text);background:var(--danger-bg);border-color:var(--danger-border)}\
button.danger:hover{background:var(--danger-hover);border-color:var(--danger-text)}\
.muted{color:var(--text-muted)}\
.badge{display:inline-block;min-width:20px;padding:0 var(--space-2);color:var(--text);font-size:12px;font-weight:600;\
line-height:20px;text-align:center;background:var(--surface-3);border-radius:10px}\
.thumbs{display:flex;flex-wrap:wrap;gap:var(--space-2);margin:var(--space-2) 0}\
.thumbs:empty{display:none}\
.thumbs a{display:block;border-radius:var(--radius-md)}\
.thumbs img{display:block;max-width:160px;max-height:120px;background:var(--bg);\
border:1px solid var(--border-strong);border-radius:var(--radius-md)}\
.notice,.error{max-width:560px;margin:var(--space-6) auto;padding:var(--space-5);background:var(--surface-1);\
border:1px solid var(--border);border-radius:var(--radius-lg)}\
.notice h1,.error h1{margin-bottom:var(--space-3);font-size:20px}\
.notice p,.error p{margin:0 0 var(--space-3)}\
.notice p:last-child,.error p:last-child{margin-bottom:0}\
.error{border-color:var(--danger-border)}\
.error h1{color:var(--danger-text)}\
@media (max-width:640px){\
header.admin-header{padding:var(--space-3) var(--space-4)}\
main.admin-main{padding:var(--space-4)}\
form.admin-form{display:flex;width:100%;margin-right:0}\
header.admin-header form.admin-form{display:inline-flex;width:auto}\
form.admin-form input[type=text]{flex:1 1 100%;width:auto}\
table.admin-table{display:block;overflow-x:auto}\
.notice,.error{margin:var(--space-4)}}\
@media (prefers-reduced-motion:reduce){*{transition:none!important}button:active,a.button:active{transform:none}}\
";
