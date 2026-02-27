pub static HEAD: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Информация о подписке</title>
<style>
body {
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    background: #0f172a;
    color: #e5e7eb;
    max-width: 720px;
    margin: 40px auto;
    padding: 24px;
}
.card {
    background: #020617;
    border-radius: 12px;
    padding: 24px;
    box-shadow: 0 0 0 1px #1e293b;
}
h1, h2, h3 {
    margin-top: 0;
}
.stat {
    margin: 8px 0;
}
.status-active {
    color: #22c55e;
    font-weight: 600;
}
.status-expired {
    color: #ef4444;
    font-weight: 600;
}
a {
    color: #38bdf8;
    text-decoration: none;
}
button {
    margin-top: 6px;
    padding: 6px 12px;
    border-radius: 6px;
    border: none;
    background: #1e293b;
    color: #e5e7eb;
    cursor: pointer;
}
button:hover {
    background: #334155;
}
button-id:hover {
    background: #334155;
    font-size: 10px;

}
.qr {
    margin-top: 16px;
    text-align: center;
}
.small {
    font-size: 13px;
    color: #94a3b8;
}
.small-id {
    font-size: 11px;
    color: #94a3b8;
    text-align: right;
    margin-top: 0px;
}
.small-viva {
    font-size: 13px;
    color: #94a3b8;
    text-align: right;
    margin-top: 0px;
}
.badge {
        font-size: 12px;
        color: var(--accent-2);
        border: 1px solid rgba(34, 211, 238, 0.3);
        padding: 4px 10px;
        border-radius: 999px;
      }
.footer-line {
    display: flex;
    justify-content: flex-end;
    align-items: center;            
    margin-top: 8px;
}
ul {
    padding-left: 20px;
}

.proxy-list {
    list-style: none;
    padding: 0;
    margin: 20px 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
}
.proxy-item {
    border-radius: 14px;
    overflow: hidden;
    transition: all 0.25s ease;
}

.proxy-item a {
    display: flex;
    justify-content: space-between;
    align-items: center;

    padding: 16px 18px;

    text-decoration: none;
    color: #e5e7eb;

    background: rgba(15, 23, 42, 0.6);
    border: 1px solid rgba(148, 163, 184, 0.15);

    backdrop-filter: blur(8px);
    border-radius: 14px;

    transition: all 0.25s ease;
}

/* hover glow */
.proxy-item a:hover {
    background: rgba(30, 41, 59, 0.9);
    border-color: rgba(56, 189, 248, 0.6);
    transform: translateY(-2px);
    box-shadow:
        0 0 0 1px rgba(56,189,248,0.3),
        0 8px 24px rgba(0,0,0,0.4);
}

/* label */
.proxy-label {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.3px;
}

/* connect badge */
.proxy-action {
    font-size: 13px;
    padding: 6px 12px;
    border-radius: 999px;

    background: linear-gradient(
        135deg,
        #38bdf8,
        #818cf8
    );

    color: #020617;
    font-weight: 600;
}
</style>
</head>"#;

pub static FOOTER: &str = r#"
<div class=footer-line>
<div class=small-viva> 
 <a href=https://t.me/frkn_support>Поддержка &nbsp·</a></div>

<div class=small-viva > &nbsp Vive la résistance! </div>
</div>"#;
