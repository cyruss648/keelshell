/* Static visual exploration. All data below is synthetic; no SSH, model,
 * storage, upload, telemetry or command-execution API is used by this demo. */
(() => {
  "use strict";
  const root = document.documentElement.dataset.root || ".";
  const initialTheme = document.documentElement.dataset.theme || "frost";
  const app = document.querySelector("#app");
  const themes = {
    frost: {
      title: "A · 霜白", english: "Frost", folder: "a-frost",
      description: "清晰、轻盈的浅色工作区",
      details: [
        "以生产力应用的轻量列表为参考，冷白主区与灰蓝侧栏建立层次，深色终端形成明确的阅读区域。",
        "6px 控件圆角、细分隔线、蓝色选中态。连接显示两行身份，文件表格保持紧凑。",
        "适合白天、浅色系统主题，以及希望保留熟悉桌面工具秩序的使用习惯。",
      ],
      native: "主要映射到 Palette、Kit Button/Input、列表行和分隔线，实现复杂度较低。",
    },
    graphite: {
      title: "B · 曜石", english: "Graphite", folder: "b-graphite",
      description: "紧凑、沉静的深色工作台",
      details: [
        "参考深色项目协作界面的明度层次，暖黑与石墨灰衔接终端、侧栏和文件区。",
        "4px 控件圆角、更紧凑的单行连接、琥珀色主动作。深浅差与文字权重承担层次。",
        "适合深色桌面、较长时间的终端操作，以及希望界面尽量退到内容之后的使用习惯。",
      ],
      native: "以实色面板、细边框和现有语义主题为主，实现复杂度较低。",
    },
    lavender: {
      title: "C · 雾紫", english: "Lavender", folder: "c-lavender",
      description: "柔和、有个性的创意工作区",
      details: [
        "借鉴创意工具的单色系材料和浏览器工作区分组，以低饱和紫灰作为底色。",
        "12px 控件圆角、独立面板间距、更宽的连接栏。终端使用深梅紫，语法颜色保持可辨认。",
        "适合希望工具更有个人风格、面板层次更柔和的使用习惯。",
      ],
      native: "圆角裁切、面板间距和状态色均可映射 GPUI；窄窗口与多分屏需要额外布局验证。",
    },
  };
  const theme = themes[initialTheme] || themes.frost;
  const paths = {
    server: '<rect x="3" y="3" width="18" height="7" rx="2"/><rect x="3" y="14" width="18" height="7" rx="2"/><path d="M7 6.5h.01M7 17.5h.01M11 6.5h6M11 17.5h6"/>',
    terminal: '<path d="m5 6 6 6-6 6M13 18h6"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10H3z"/>',
    folderOpen: '<path d="M3 9V5h7l2 2h8v3M3 10h19l-3 10H4z"/>',
    file: '<path d="M14 2H5v20h14V7zM14 2v6h5M8 13h8M8 17h6"/>',
    code: '<path d="m8 5-6 7 6 7m8-14 6 7-6 7M14 3l-4 18"/>',
    search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    close: '<path d="m6 6 12 12M6 18 18 6"/>',
    chevron: '<path d="m9 5 7 7-7 7"/>',
    down: '<path d="m5 9 7 7 7-7"/>',
    upload: '<path d="M12 16V3m-5 5 5-5 5 5M4 15v6h16v-6"/>',
    download: '<path d="M12 3v13m-5-5 5 5 5-5M4 18v3h16v-3"/>',
    refresh: '<path d="M20 7a9 9 0 1 0 1 8M20 3v5h-5"/>',
    sparkles: '<path d="m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5zM20 2v4M18 4h4"/>',
    panel: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>',
    split: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/>',
    expand: '<path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/>',
    settings: '<path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V6a4 4 0 0 1 8 0v4M12 14v3"/>',
    check: '<path d="m5 12 4 4L19 6"/>',
    arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
    network: '<rect x="8" y="2" width="8" height="6" rx="1"/><rect x="2" y="16" width="7" height="6" rx="1"/><rect x="15" y="16" width="7" height="6" rx="1"/><path d="M12 8v4M5 16v-4h14v4"/>',
    book: '<path d="M4 3h6a3 3 0 0 1 3 3v15a4 4 0 0 0-4-2H4zM13 6a3 3 0 0 1 3-3h5v16h-5a4 4 0 0 0-3 2"/>',
    activity: '<path d="M2 12h4l3-8 6 16 3-8h4"/>',
    queue: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
    ellipsis: '<path d="M4 12h.01M12 12h.01M20 12h.01"/>',
    pin: '<path d="m16 3 5 5-4 1-4 4v4l-6-6h4l4-4zM8 16l-5 5"/>',
  };
  const icon = (name) => `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.file}</svg>`;
  const button = (name, label, action, classes = "") =>
    `<button type="button" class="${classes}" data-action="${action}" aria-label="${label}">${icon(name)}<span>${label}</span></button>`;
  const iconButton = (name, label, action, extra = "") =>
    `<button type="button" class="icon-button" aria-label="${label}" title="${label}" data-action="${action}" ${extra}>${icon(name)}</button>`;
  const hosts = [
    { key: "web-01", name: "web-01", address: "web-01.example.com", user: "deploy", group: "production", active: true },
    { key: "api-01", name: "api-01", address: "api-01.example.com", user: "deploy", group: "production", active: false },
    { key: "database", name: "database", address: "db.example.com", user: "operator", group: "production", active: false },
    { key: "staging", name: "staging", address: "stage.example.com", user: "deploy", group: "development", active: false },
    { key: "sandbox", name: "sandbox", address: "sandbox.example.com", user: "demo", group: "development", active: false },
  ];
  const hostGroups = { production: "生产环境", development: "开发环境" };
  const files = [
    { name: "conf.d", size: "4 KB", type: "文件夹", modified: "10:42", icon: "folder", dir: true },
    { name: "sites-enabled", size: "4 KB", type: "文件夹", modified: "10:42", icon: "folder", dir: true },
    { name: "nginx.conf", size: "2.4 KB", type: "配置文件", modified: "10:38", icon: "code" },
    { name: "mime.types", size: "5.3 KB", type: "配置文件", modified: "昨天", icon: "file" },
    { name: "proxy_params", size: "1.1 KB", type: "配置文件", modified: "昨天", icon: "file" },
  ];
  const themeLinks = Object.entries(themes).map(([key, value], index) =>
    `<a class="theme-option" href="${root}/${value.folder}/index.html"${key === initialTheme ? ' aria-current="page"' : ""}><span class="swatch swatch-${"abc"[index]}" aria-hidden="true"></span>${value.title}<span class="theme-en">${value.english}</span></a>`).join("");
  const connection = (host) => `<button type="button" class="connection" data-host="${host.key}" aria-pressed="${host.active}" title="${host.address}">${icon("server")}<span class="connection-copy"><strong>${host.name}</strong><small>${host.address}</small></span><span class="signal-dot ${host.active ? "" : "off"}" aria-hidden="true"></span></button>`;
  const group = (key) => `<div class="connection-group"><div class="group-label">${icon("down")}${icon("folder")}<span>${hostGroups[key]}</span><span>${hosts.filter(h => h.group === key).length}</span></div>${hosts.filter(h => h.group === key).map(connection).join("")}</div>`;
  const sessionTab = (host) => `<button type="button" class="session-tab" id="session-${host.key}" data-session="${host.key}" role="tab" aria-controls="terminal-view" aria-selected="${host.active}" tabindex="${host.active ? 0 : -1}">${host.active ? '<span class="signal-dot" aria-hidden="true"></span>' : icon("terminal")}${host.name} <span class="tab-close">SSH</span></button>`;
  const table = files.map((file) => `<tr class="file-row" data-file="${file.name}" tabindex="0" aria-selected="${file.name === "nginx.conf"}"><td><span class="file-name">${icon(file.icon)}${file.name}</span></td><td class="mono">${file.size}</td><td>${file.type}</td><td class="mono">${file.modified}</td></tr>`).join("");

  app.innerHTML = `
  <a class="skip-link" href="#workspace-content">跳到工作区</a>
  <header class="demo-strip">
    <div class="demo-title"><h1>KeelShell 主题探索</h1><span>选择一种你愿意每天打开的界面</span></div>
    <nav class="theme-picker" aria-label="主题方案">${themeLinks}</nav>
    <div class="demo-note"><span>HTML 原型 / 示例数据</span><button type="button" class="text-link" data-action="design">方案说明 ${icon("arrow")}</button></div>
  </header>
  <section class="window" aria-label="${theme.title} 交互原型">
    <div class="workspace-heading">
      <div class="brand"><img src="${document.documentElement.dataset.icon}" width="20" height="20" alt="">KeelShell</div>
      <div class="window-context">${icon("folder")} <span>我的工作区</span><span>/</span><span>远程主机</span></div>
      <div class="window-tools"><span class="tag">视觉原型</span>${iconButton("settings", "外观与布局", "settings")}</div>
    </div>
    <div class="toolbar">
      ${iconButton("panel", "显示或隐藏连接导航", "toggle-nav", 'aria-pressed="true"')}
      <div class="toolbar-divider"></div>
      <div class="breadcrumb"><span class="breadcrumb-parent">生产环境</span>${icon("chevron")}<strong class="active-host-name">web-01</strong><span class="tag">SSH</span></div>
      <div class="spacer"></div>
      ${button("plus", "新建连接", "new-connection", "desktop-action")}
      ${button("split", "分屏", "split", "desktop-action")}
      ${iconButton("expand", "专注终端", "focus", 'aria-pressed="false"')}
      <div class="toolbar-divider"></div>
      ${button("sparkles", "AI 助手", "toggle-ai", "toggle-button")}
    </div>
    <div class="workbench" id="workspace-content" tabindex="-1">
      <aside class="sidebar" aria-label="连接与主机状态">
        <div class="sidebar-top">
          <div class="region-heading"><h2>连接</h2>${iconButton("plus", "添加连接", "new-connection")}</div>
          <label class="search">${icon("search")}<input id="connection-search" name="connection-search" aria-label="搜索示例连接" placeholder="搜索连接…" autocomplete="off"></label>
        </div>
        <div class="connection-tree">${group("production")}${group("development")}<p class="empty-search" hidden>没有匹配的示例连接。</p></div>
        <section class="host-health" aria-label="示例主机监控">
          <div class="region-heading"><h3>主机概览</h3><span class="tag">示例</span></div>
          <div class="health-meta"><span>Ubuntu 24.04 LTS</span><span>在线 14 天</span></div>
          <div class="metric"><span>CPU</span><strong>12.4 <small>%</small></strong></div><div class="meter"><span style="--value: 12.4%"></span></div>
          <div class="metric"><span>内存</span><strong>2.8 <small>/ 8 GB</small></strong></div><div class="meter"><span style="--value: 35%"></span></div>
          <div class="metric"><span>磁盘</span><strong>36 <small>/ 100 GB</small></strong></div><div class="meter"><span style="--value: 36%"></span></div>
          <svg class="network-chart" viewBox="0 0 190 35" preserveAspectRatio="none" role="img" aria-label="虚构网络流量曲线"><polyline points="0,29 8,27 16,28 24,21 32,24 40,22 48,26 56,14 64,19 72,16 80,24 88,22 96,9 104,16 112,14 120,21 128,18 136,25 144,19 152,7 160,17 168,13 176,22 184,18 190,20"/></svg>
          <div class="network-legend"><span>↓ 124 KB/s</span><span>↑ 32 KB/s</span></div>
        </section>
        <div class="sidebar-footer">${icon("lock")}<span>连接与凭据分开管理</span></div>
      </aside>
      <section class="center" aria-label="终端与文件">
        <div class="session-strip" role="tablist" aria-label="示例会话">
          ${hosts.slice(0, 2).map(sessionTab).join("")}
          ${iconButton("plus", "打开新会话", "new-connection")}
          <div class="spacer"></div>${iconButton("ellipsis", "会话信息", "session-info")}
        </div>
        <div class="terminal-shell" id="terminal-view" role="tabpanel" aria-labelledby="session-web-01">
          <div class="terminal-caption">${icon("lock")}<span class="terminal-endpoint">deploy@web-01.example.com</span><span class="protocol-detail">SSH-2 · UTF-8</span><span>演示会话</span></div>
          <div class="terminal-scroll" aria-label="示例终端输出"></div>
        </div>
        <form class="commandbar" id="command-form">
          <label class="command-prefix" for="command-input">❯</label>
          <input id="command-input" name="command" aria-label="命令草稿，不会执行" value="nginx -t" spellcheck="false" autocomplete="off">
          ${iconButton("clock", "打开命令历史示例", "history")}
          <button type="submit" class="primary">${icon("arrow")}<span>预览命令</span></button>
        </form>
        <section class="dock" aria-label="会话工具">
          <div class="dock-header">
            <div class="dock-tabs" role="tablist" aria-label="底部工具">
              <button type="button" class="dock-tab" role="tab" aria-controls="pane-files" data-dock="files" aria-selected="true">${icon("folderOpen")}文件</button>
              <button type="button" class="dock-tab" role="tab" aria-controls="pane-commands" data-dock="commands" aria-selected="false">${icon("terminal")}命令</button>
              <button type="button" class="dock-tab" role="tab" aria-controls="pane-tunnels" data-dock="tunnels" aria-selected="false">${icon("network")}隧道</button>
            </div>
            <div class="spacer"></div><button type="button" class="queue-button" data-action="queue">${icon("queue")}传输队列 <span class="queue-count">0</span></button>
          </div>
          <div class="files-pane" id="pane-files" role="tabpanel">
            <div class="file-toolbar">
              ${iconButton("refresh", "刷新文件列表示例", "refresh")}
              <span class="file-path">${icon("folder")}<span id="file-path">/etc/nginx</span></span>
              ${button("upload", "上传", "upload", "file-action")}
              ${button("download", "下载", "download", "file-action")}
              ${iconButton("ellipsis", "文件工具", "file-tools")}
            </div>
            <div class="files-body">
              <nav class="directory-tree" aria-label="示例远端目录">
                <button type="button" class="directory-row" data-directory="/">${icon("down")}${icon("folder")}根目录</button>
                <button type="button" class="directory-row indent" data-directory="/home">${icon("folder")}home</button>
                <button type="button" class="directory-row indent" data-directory="/etc">${icon("folder")}etc</button>
                <button type="button" class="directory-row indent selected" data-directory="/etc/nginx">${icon("folderOpen")}nginx</button>
                <button type="button" class="directory-row indent" data-directory="/var">${icon("folder")}var</button>
              </nav>
              <div class="file-list"><table class="file-table" aria-label="示例远端文件"><thead><tr><th>名称</th><th>大小</th><th>类型</th><th>修改时间</th></tr></thead><tbody>${table}</tbody></table></div>
            </div>
            <div class="file-footer"><span id="file-selection">5 个项目 · 已选 nginx.conf</span><span>双击预览文件</span></div>
          </div>
          <div class="tool-pane" id="pane-commands" role="tabpanel" hidden>
            <p>点击“填入草稿”体验命令准备流程，不执行任何操作。</p>
            <div class="snippet"><div><strong>检查服务配置</strong><code>nginx -t</code></div><button type="button" data-command="nginx -t">填入草稿 ${icon("arrow")}</button></div>
            <div class="snippet"><div><strong>查看监听端口</strong><code>ss -lntp</code></div><button type="button" data-command="ss -lntp">填入草稿 ${icon("arrow")}</button></div>
            <div class="snippet"><div><strong>磁盘使用情况</strong><code>df -h</code></div><button type="button" data-command="df -h">填入草稿 ${icon("arrow")}</button></div>
          </div>
          <div class="tool-pane" id="pane-tunnels" role="tabpanel" hidden>
            <p>命名规则示例。此页面没有创建网络监听器。</p>
            <div class="snippet"><div><strong>database-console</strong><code>127.0.0.1:15432 → 127.0.0.1:5432</code></div><span class="tunnel-status">未启动</span></div>
            <div class="snippet"><div><strong>preview-service</strong><code>127.0.0.1:8080 → 127.0.0.1:80</code></div><span class="tunnel-status">未启动</span></div>
          </div>
        </section>
      </section>
      <aside class="assistant" aria-label="AI 助手原型">
        <div class="assistant-heading">${icon("sparkles")}<h2>AI 助手</h2><div class="spacer"></div><span class="tag">示例</span>${iconButton("close", "收起 AI 助手", "toggle-ai")}</div>
        <div class="assistant-nav" role="tablist" aria-label="AI 面板">
          <button type="button" role="tab" data-assistant="chat" aria-controls="assistant-chat" aria-selected="true">会话助手</button>
          <button type="button" role="tab" data-assistant="knowledge" aria-controls="assistant-knowledge" aria-selected="false">本地知识</button>
        </div>
        <div class="assistant-content" id="assistant-chat" role="tabpanel">
          <div class="assistant-context">${icon("pin")}<span>仅使用手动选择的上下文</span></div>
          <div class="prompt-bubble">帮我检查这台服务器的 nginx 配置，先给我一个只读检查步骤。</div>
          <div class="response-heading">${icon("sparkles")}检查建议 <span class="tag">演示回答</span></div>
          <p>可以先检查配置语法，再查看服务状态。建议先确认目标主机与命令内容。</p>
          <h3>01　检查语法</h3>
          <div class="code-suggestion">nginx -t</div>
          <button type="button" class="suggestion-action" data-command="nginx -t">${icon("arrow")}放入命令草稿</button>
          <details class="next-step"><summary>下一步：读取服务状态</summary><div class="code-suggestion">systemctl status nginx --no-pager</div></details>
          <div class="assistant-footnote">草稿需要再次审核；本页不会执行命令或调用模型。</div>
        </div>
        <div class="assistant-content" id="assistant-knowledge" role="tabpanel" hidden>
          <p>本地知识的样式示例</p>
          <div class="knowledge-row"><strong>nginx 配置检查</strong><small>手动整理 / 示例条目</small></div>
          <div class="knowledge-row"><strong>服务排障顺序</strong><small>手动整理 / 示例条目</small></div>
          <div class="knowledge-row"><strong>部署前检查清单</strong><small>手动整理 / 示例条目</small></div>
        </div>
        <form class="composer" id="assistant-form">
          <label class="context-choice"><input type="checkbox" name="include-context">附带我选中的终端内容（演示）</label>
          <textarea name="prompt" aria-label="AI 提问草稿，仅演示" placeholder="描述问题，或询问下一步…"></textarea>
          <div class="composer-footer"><span>未连接模型</span><button type="submit">${icon("arrow")}预览提问</button></div>
        </form>
      </aside>
    </div>
    <footer class="statusbar"><span class="good">${icon("lock")}演示连接</span><span>SSH / UTF-8</span><span class="optional">SFTP 示例</span><div class="spacer"></div><span>不连接服务器，不执行命令</span><span class="optional">${icon("book")}本地原型</span></footer>
  </section>
  <div class="prototype-caption"><span><strong>${theme.title} ${theme.english}</strong>　${theme.description}</span><span>试试切换连接、文件/命令标签、AI 面板，或在右上角调整密度。</span></div>
  <dialog id="dialog" aria-labelledby="dialog-title"><h2 id="dialog-title"></h2><div id="dialog-content"></div><div class="dialog-actions"><button type="button" class="secondary" data-action="close-dialog">返回工作区</button></div></dialog>`;

  const windowEl = document.querySelector(".window");
  const dialog = document.querySelector("#dialog");
  const dialogContent = document.querySelector("#dialog-content");
  const commandInput = document.querySelector("#command-input");
  const panelBreakpoints = { nav: "(max-width: 760px)", ai: "(max-width: 1060px)" };
  let currentHost = hosts[0];
  let selectedFile = "nginx.conf";
  let currentDirectory = "/etc/nginx";
  let refreshTimer;

  function showDialog(title, content, { html = false } = {}) {
    document.querySelector("#dialog-title").textContent = title;
    dialogContent.replaceChildren();
    if (html) dialogContent.innerHTML = content;
    else {
      const paragraph = document.createElement("p");
      paragraph.textContent = content;
      dialogContent.append(paragraph);
    }
    if (!dialog.open) dialog.showModal();
  }

  function renderTerminal() {
    const lines = [
      `<span class="term-muted">KeelShell theme preview · synthetic session</span>`,
      `<span class="term-muted">Ubuntu 24.04 LTS   /   ${currentHost.name}   /   14 days uptime</span>`,
      "",
      `<span class="term-prompt">${currentHost.user}@${currentHost.name}</span> <span class="term-path">/etc/nginx</span> <span class="term-muted">❯</span> ls -lh`,
      `<span class="term-muted">total 24K</span>`,
      `<span class="term-muted">drwxr-xr-x   root root   4.0K</span>  <span class="term-path">conf.d/</span>`,
      `<span class="term-muted">drwxr-xr-x   root root   4.0K</span>  <span class="term-path">sites-enabled/</span>`,
      `<span class="term-muted">-rw-r--r--   root root   2.4K</span>  nginx.conf`,
      `<span class="term-muted">-rw-r--r--   root root   5.3K</span>  mime.types`,
      "",
      `<span class="term-prompt">${currentHost.user}@${currentHost.name}</span> <span class="term-path">/etc/nginx</span> <span class="term-muted">❯</span> <span class="cursor"></span>`,
    ];
    document.querySelector(".terminal-scroll").innerHTML = lines.map(line => `<span class="terminal-line">${line || " "}</span>`).join("");
  }
  renderTerminal();

  function setHost(key) {
    currentHost = hosts.find(host => host.key === key) || hosts[0];
    let activeTab = document.querySelector(`[data-session="${currentHost.key}"]`);
    if (!activeTab) {
      document.querySelector('.session-strip [data-action="new-connection"]').insertAdjacentHTML("beforebegin", sessionTab(currentHost));
      activeTab = document.querySelector(`[data-session="${currentHost.key}"]`);
    }
    document.querySelectorAll("[data-host]").forEach(el => {
      const active = el.dataset.host === currentHost.key;
      el.setAttribute("aria-pressed", String(active));
      el.querySelector(".signal-dot").classList.toggle("off", !active);
    });
    document.querySelectorAll("[data-session]").forEach(el => {
      const active = el.dataset.session === currentHost.key;
      el.setAttribute("aria-selected", String(active));
      el.tabIndex = active ? 0 : -1;
    });
    document.querySelector(".breadcrumb-parent").textContent = hostGroups[currentHost.group];
    document.querySelectorAll(".active-host-name").forEach(el => { el.textContent = currentHost.name; });
    document.querySelector(".terminal-endpoint").textContent = `${currentHost.user}@${currentHost.address}`;
    document.querySelector("#terminal-view").setAttribute("aria-labelledby", activeTab.id);
    renderTerminal();
    activeTab.scrollIntoView({ block: "nearest", inline: "nearest" });
  }
  function selectDock(key) {
    windowEl.classList.remove("focus-terminal");
    document.querySelector(".dock").hidden = false;
    document.querySelectorAll("[data-dock]").forEach(el => el.setAttribute("aria-selected", String(el.dataset.dock === key)));
    ["files", "commands", "tunnels"].forEach(name => { document.querySelector(`#pane-${name}`).hidden = name !== key; });
    updateToggles();
  }
  function selectAssistant(key) {
    document.querySelectorAll("[data-assistant]").forEach(el => el.setAttribute("aria-selected", String(el.dataset.assistant === key)));
    ["chat", "knowledge"].forEach(name => { document.querySelector(`#assistant-${name}`).hidden = name !== key; });
  }
  function setDraft(value) {
    commandInput.value = value;
    commandInput.focus({ preventScroll: true });
    commandInput.select();
  }
  function panelIsVisible(name) {
    if (windowEl.classList.contains("focus-terminal")) return false;
    return matchMedia(panelBreakpoints[name]).matches
      ? windowEl.classList.contains(`mobile-${name}-open`)
      : !windowEl.classList.contains(`${name}-closed`);
  }
  function dockIsVisible() {
    return !windowEl.classList.contains("focus-terminal") && !document.querySelector(".dock").hidden;
  }
  function updateToggles() {
    document.querySelectorAll('[data-action="toggle-ai"]').forEach(el => el.setAttribute("aria-pressed", String(panelIsVisible("ai"))));
    document.querySelector('[data-action="toggle-nav"]').setAttribute("aria-pressed", String(panelIsVisible("nav")));
    document.querySelector('[data-action="focus"]').setAttribute("aria-pressed", String(windowEl.classList.contains("focus-terminal")));
    const dockToggle = document.querySelector("#dock-toggle");
    if (dockToggle) dockToggle.checked = dockIsVisible();
  }
  function togglePanel(name) {
    const visible = panelIsVisible(name);
    windowEl.classList.remove("focus-terminal");
    if (matchMedia(panelBreakpoints[name]).matches) windowEl.classList.toggle(`mobile-${name}-open`, !visible);
    else windowEl.classList.toggle(`${name}-closed`, visible);
    updateToggles();
  }
  updateToggles();
  window.addEventListener("resize", updateToggles);

  function previewFile(name) {
    const file = files.find(item => item.name === name);
    if (!file || currentDirectory !== "/etc/nginx") return;
    selectFile(name);
    if (file.dir) {
      setDirectory(`${currentDirectory}/${name}`.replace(/\/+/g, "/"));
      return;
    }
    const sample = name === "nginx.conf"
      ? "# 仅用于界面预览的示例配置\nuser www-data;\nworker_processes auto;\n\nhttp {\n    include /etc/nginx/mime.types;\n    sendfile on;\n    keepalive_timeout 65;\n    include /etc/nginx/conf.d/*.conf;\n}"
      : "# 示例文件预览\n# 本地 HTML 原型，没有读取任何远端文件。";
    showDialog(`${name} · 只读示例`, "");
    const pre = document.createElement("pre");
    pre.className = "file-preview";
    pre.textContent = sample;
    dialogContent.replaceChildren(pre);
  }
  function updateFileSelection() {
    const atBase = currentDirectory === "/etc/nginx";
    if (!atBase || !files.some(file => file.name === selectedFile)) selectedFile = null;
    document.querySelectorAll("[data-file]").forEach(el => el.setAttribute("aria-selected", String(el.dataset.file === selectedFile)));
    document.querySelector('[data-action="download"]').disabled = selectedFile === null;
    document.querySelector("#file-selection").textContent = atBase
      ? `${files.length} 个项目 · ${selectedFile ? `已选 ${selectedFile}` : "未选择文件"}`
      : "空目录示例";
  }
  function selectFile(name) {
    if (currentDirectory !== "/etc/nginx") return;
    selectedFile = name;
    updateFileSelection();
  }
  function setDirectory(path) {
    if (path !== currentDirectory) selectedFile = null;
    currentDirectory = path;
    document.querySelector("#file-path").textContent = path;
    document.querySelectorAll("[data-directory]").forEach(el => el.classList.toggle("selected", el.dataset.directory === path));
    const atBase = path === "/etc/nginx";
    document.querySelector(".file-table").hidden = !atBase;
    let empty = document.querySelector("#directory-empty");
    if (!empty) {
      empty = document.createElement("div");
      empty.id = "directory-empty";
      empty.className = "tool-pane";
      empty.innerHTML = '<p>此目录未准备演示内容。</p><button type="button" class="secondary" data-directory="/etc/nginx">回到 /etc/nginx</button>';
      document.querySelector(".file-list").append(empty);
    }
    empty.hidden = atBase;
    updateFileSelection();
  }
  updateFileSelection();

  const actions = {
    "toggle-nav": () => togglePanel("nav"),
    "toggle-ai": () => togglePanel("ai"),
    focus: () => {
      windowEl.classList.toggle("focus-terminal");
      updateToggles();
    },
    split: () => showDialog("分屏布局", "正式应用中的终端分屏会保留每个会话的独立目标。本次主题原型先展示单终端与文件同屏的视觉效果。"),
    "new-connection": () => showDialog("新建连接", '<p>这里展示连接表单的组件样式。此原型不会保存配置或建立连接。</p><label class="settings-row">名称 <input name="demo-name" aria-label="示例连接名称" value="sandbox"></label><label class="settings-row">主机 <input name="demo-host" aria-label="示例主机" value="sandbox.example.com"></label><label class="settings-row">用户名 <input name="demo-user" aria-label="示例用户名" value="demo"></label>', { html: true }),
    "session-info": () => showDialog("当前示例会话", `${currentHost.user}@${currentHost.address}。这只是配色与布局预览，没有真实连接、认证状态或监控采样。`),
    history: () => { selectDock("commands"); },
    queue: () => showDialog("传输队列", "队列为空。此页面没有上传或下载文件；这里展示空状态和对话框的主题。"),
    upload: () => showDialog("上传文件", "正式应用会先准备上传计划，再显示目标与冲突审核。本原型不打开文件选择器，也不会传输本机文件。"),
    download: () => {
      if (currentDirectory !== "/etc/nginx" || selectedFile === null) {
        showDialog("未选择文件", "请先选择一个示例项目。本原型不会从服务器读取数据。");
        return;
      }
      showDialog("下载文件", `当前选中 ${currentDirectory}/${selectedFile}。本原型只展示下载入口，不会从服务器读取数据。`);
    },
    "file-tools": () => showDialog("文件工具", '<p>高级操作集中在按需打开的工具面板，减少默认浏览区的按钮占用。</p><div class="snippet"><span>目录比较</span><span class="tag">只读准备</span></div><div class="snippet"><span>文件同步</span><span class="tag">审核后执行</span></div><div class="snippet"><span>修改权限</span><span class="tag">确认具体目标</span></div>', { html: true }),
    refresh: (el) => {
      clearTimeout(refreshTimer);
      el.disabled = true;
      el.setAttribute("aria-busy", "true");
      document.querySelector("#file-selection").textContent = "演示加载状态…";
      refreshTimer = setTimeout(() => {
        el.disabled = false;
        el.removeAttribute("aria-busy");
        updateFileSelection();
      }, 650);
    },
    settings: () => showDialog("外观与布局", `<p>${theme.title} ${theme.english}。调整下面的布局开关，观察界面密度变化。</p><label class="settings-row">紧凑密度 <input type="checkbox" id="compact-toggle"${windowEl.classList.contains("compact") ? " checked" : ""}></label><label class="settings-row">显示文件工具区 <input type="checkbox" id="dock-toggle"${dockIsVisible() ? " checked" : ""}></label><div class="palette-swatches" aria-label="当前主题的侧栏、表面、终端与强调色"><span></span><span></span><span></span><span></span></div><p class="assistant-footnote">所有选择只作用于当前页面，刷新后恢复。</p>`, { html: true }),
    design: () => showDialog(`${theme.title} ${theme.english}`, `<p>${theme.description}</p><ul class="design-points">${theme.details.map(line => `<li>${line}</li>`).join("")}</ul><h3>GPUI 落地判断</h3><p>${theme.native}</p><div class="palette-swatches" aria-label="主题色"><span></span><span></span><span></span><span></span></div><p class="assistant-footnote">这是一项实现预估，未经过三平台原生验证。主题确认后再映射到正式组件。</p>`, { html: true }),
    "close-dialog": () => { dialog.close(); },
  };

  document.addEventListener("click", event => {
    const action = event.target.closest("[data-action]");
    if (action && actions[action.dataset.action]) {
      if (action.disabled) return;
      actions[action.dataset.action](action);
      return;
    }
    const host = event.target.closest("[data-host]");
    if (host) setHost(host.dataset.host);
    const session = event.target.closest("[data-session]");
    if (session) setHost(session.dataset.session);
    const dock = event.target.closest("[data-dock]");
    if (dock) selectDock(dock.dataset.dock);
    const assistant = event.target.closest("[data-assistant]");
    if (assistant) selectAssistant(assistant.dataset.assistant);
    const command = event.target.closest("[data-command]");
    if (command) setDraft(command.dataset.command);
    const directory = event.target.closest("[data-directory]");
    if (directory) setDirectory(directory.dataset.directory);
    const file = event.target.closest("[data-file]");
    if (file) selectFile(file.dataset.file);
  });
  document.addEventListener("dblclick", event => {
    const file = event.target.closest("[data-file]");
    if (file) previewFile(file.dataset.file);
  });
  document.addEventListener("keydown", event => {
    const file = event.target.closest("[data-file]");
    if (file && ["Enter", " "].includes(event.key)) {
      event.preventDefault();
      if (event.key === "Enter") previewFile(file.dataset.file);
      else selectFile(file.dataset.file);
    }
    const tab = event.target.closest('[role="tab"]');
    if (tab && ["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
      const siblings = [...tab.closest('[role="tablist"]').querySelectorAll('[role="tab"]')];
      const index = siblings.indexOf(tab);
      const next = event.key === "Home" ? 0 : event.key === "End" ? siblings.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + siblings.length) % siblings.length;
      event.preventDefault();
      siblings[next].focus();
      siblings[next].click();
    }
  });
  document.addEventListener("change", event => {
    if (event.target.id === "compact-toggle") windowEl.classList.toggle("compact", event.target.checked);
    if (event.target.id === "dock-toggle") {
      document.querySelector(".dock").hidden = !event.target.checked;
      if (event.target.checked) windowEl.classList.remove("focus-terminal");
      updateToggles();
    }
  });
  document.querySelector("#connection-search").addEventListener("input", event => {
    const query = event.target.value.toLowerCase().trim();
    let count = 0;
    document.querySelectorAll("[data-host]").forEach(el => {
      const host = hosts.find(item => item.key === el.dataset.host);
      const visible = `${host.name} ${host.address}`.includes(query);
      el.hidden = !visible;
      if (visible) count += 1;
    });
    document.querySelector(".empty-search").hidden = count !== 0;
  });
  document.querySelector("#command-form").addEventListener("submit", event => {
    event.preventDefault();
    const command = commandInput.value.trim();
    showDialog(command ? "审核命令草稿" : "命令草稿为空", "");
    const note = document.createElement("p");
    note.textContent = command ? `示例目标：${currentHost.user}@${currentHost.address}。此原型只展示审核样式，没有执行按钮。` : "请在命令栏输入内容后再预览。";
    dialogContent.replaceChildren(note);
    if (command) {
      const pre = document.createElement("pre");
      pre.className = "dialog-code";
      pre.textContent = command;
      dialogContent.append(pre);
    }
  });
  document.querySelector("#assistant-form").addEventListener("submit", event => {
    event.preventDefault();
    const prompt = event.target.elements.prompt.value.trim();
    const include = event.target.elements["include-context"].checked;
    showDialog("预览提问", "");
    const note = document.createElement("p");
    note.textContent = prompt ? `未连接模型。上下文选项：${include ? "包含已选择的示例内容" : "不包含终端内容"}。下面是你的本地草稿：` : "请先输入问题；该页面不会向外部模型发送内容。";
    dialogContent.replaceChildren(note);
    if (prompt) {
      const pre = document.createElement("pre");
      pre.className = "dialog-code";
      pre.textContent = prompt;
      dialogContent.append(pre);
    }
  });
  dialog.addEventListener("click", event => {
    if (event.target === dialog) {
      const bounds = dialog.getBoundingClientRect();
      if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
    }
  });
})();
