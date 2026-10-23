/** 初始化独立原型的示例数据、全页面导航、直播间管理与交互逻辑。 */
(() => {
  const root = document.getElementById('shadowreplay-design');
  const PAGE_TITLES = {
    summary: '总览',
    rooms: '直播间',
    archives: '录播',
    materials: '素材',
    clips: '切片',
    uploads: '投稿',
    tasks: '任务',
    ai: '助手',
    accounts: '账号',
    settings: '设置',
    about: '关于',
  };

  const saved = readState();
  const design = {
    theme: saved.theme,
    density: saved.density,
    collapsed: saved.collapsed,
  };
  let currentPage = saved.page;
  let platform = '全部';
  let openMenuRoomId = null;
  let editingAutoRoomId = null;
  let toastTimer;
  let dialogTrigger;

  // 直播间示例数据（覆盖 live / offline / unknown / unconfirmed 四态及自动切片配置）
  let rooms = [
    {
      id: '22603245',
      code: 'BILIBILI',
      name: '小岛电台',
      avatar: '岛',
      title: '周五游戏夜：一起探索新地图',
      platform: 'B 站',
      status: 'live',
      recording: true,
      enabled: true,
      autoClip: true,
      threshold: 75,
      keywords: '666, 哈哈, 高能, 名场面',
      time: '已缓存 01:42:18',
      errorHint: '',
    },
    {
      id: '7391820411',
      code: 'DOUYIN',
      name: '晚风聊天室',
      avatar: '风',
      title: '聊聊这周的新鲜事',
      platform: '抖音',
      status: 'live',
      recording: true,
      enabled: true,
      autoClip: false,
      threshold: 80,
      keywords: '好听, 绝了',
      time: '已缓存 00:56:40',
      errorHint: '',
    },
    {
      id: '940128',
      code: 'BILIBILI',
      name: '像素工坊',
      avatar: '像',
      title: '从零开始的独立游戏开发',
      platform: 'B 站',
      status: 'offline',
      recording: false,
      enabled: true,
      autoClip: true,
      threshold: 70,
      keywords: 'demo, 发布, 哈哈',
      time: '等待开播',
      errorHint: '',
    },
    {
      id: '518820',
      code: 'HUYA',
      name: '山野漫游',
      avatar: '山',
      title: '周末的户外徒步计划',
      platform: '虎牙',
      status: 'offline',
      recording: false,
      enabled: false,
      autoClip: false,
      threshold: 80,
      keywords: '',
      time: '自动录制已关闭',
      errorHint: '',
    },
    {
      id: '883019245',
      code: 'DOUYIN',
      name: '深夜歌友会',
      avatar: '歌',
      title: '点歌台：经典老歌连唱',
      platform: '抖音',
      status: 'unknown',
      recording: false,
      enabled: true,
      autoClip: false,
      threshold: 80,
      keywords: '好听, 安可',
      time: '接口请求受限，下次重试 15:25',
      errorHint: '请求受限；下次重试：15:25:00',
    },
  ];

  // 录播归档示例数据
  let archives = [
    { id: 'rec-101', roomId: '22603245', roomName: '小岛电台', platform: 'B 站', title: '周五游戏夜：一起探索新地图', createdText: '今天 19:30', createdMs: 1761219000000, durationText: '01:42:18', durationSec: 6138, sizeText: '3.6 GB', sizeGb: 3.6, summaryStatus: 'success' },
    { id: 'rec-102', roomId: '7391820411', roomName: '晚风聊天室', platform: '抖音', title: '聊聊这周的新鲜事', createdText: '今天 18:00', createdMs: 1761213600000, durationText: '00:56:40', durationSec: 3400, sizeText: '1.8 GB', sizeGb: 1.8, summaryStatus: 'processing' },
    { id: 'rec-103', roomId: '940128', roomName: '像素工坊', platform: 'B 站', title: '从零开始的独立游戏开发', createdText: '昨天 20:00', createdMs: 1761134400000, durationText: '02:15:06', durationSec: 8106, sizeText: '4.2 GB', sizeGb: 4.2, summaryStatus: 'success' },
    { id: 'rec-104', roomId: '22603245', roomName: '小岛电台', platform: 'B 站', title: '周三独立游戏试玩合集', createdText: '前天 21:00', createdMs: 1761051600000, durationText: '01:28:12', durationSec: 5292, sizeText: '2.9 GB', sizeGb: 2.9, summaryStatus: 'none' },
    { id: 'rec-105', roomId: '518820', roomName: '山野漫游', platform: '虎牙', title: '秋季露营装备盘点', createdText: '3 天前', createdMs: 1760965200000, durationText: '01:12:45', durationSec: 4365, sizeText: '2.4 GB', sizeGb: 2.4, summaryStatus: 'none' },
  ];

  // 素材示例数据
  const materials = [
    { id: 'mat-1', title: '新地图隐藏 Boss 初见高能反应.mp4', roomName: '小岛电台', duration: '00:03:24', size: '128 MB', created: '今天 20:15' },
    { id: 'mat-2', title: '晚风电台听众连麦故事精选.mp4', roomName: '晚风聊天室', duration: '00:05:10', size: '186 MB', created: '今天 18:45' },
    { id: 'mat-3', title: '像素着色器调试全过程片段.mp4', roomName: '像素工坊', duration: '00:08:42', size: '312 MB', created: '昨天 22:18' },
    { id: 'mat-4', title: '高能弹幕合唱谢幕时刻.mp4', roomName: '小岛电台', duration: '00:02:15', size: '84 MB', created: '前天 22:30' },
  ];

  // 切片工程与自动切片候选示例数据
  const clipProjects = [
    { id: 'proj-1', title: '【小岛电台】周五新地图初见全高能合集', clips: 4, duration: '00:11:36', updated: '10 分钟前', note: '已包含弹幕压制与封面贴纸' },
    { id: 'proj-2', title: '【像素工坊】2 小时独立游戏开发精华浓缩', clips: 3, duration: '00:09:18', updated: '昨天 23:05', note: '待补充片尾字幕' },
    { id: 'proj-3', title: '【晚风聊天室】本周深夜故事暖心合集', clips: 2, duration: '00:06:40', updated: '2 天前', note: '已完成音频响度均衡' },
  ];

  let autoCandidates = [
    { id: 'cand-1', roomName: '小岛电台', title: '高能候选 #1：全屏“666”击败隐藏首领', range: '00:38:15 - 00:41:30', score: '96 分', status: 'pending' },
    { id: 'cand-2', roomName: '小岛电台', title: '高能候选 #2：解谜反转全场“哈哈”', range: '01:12:00 - 01:14:20', score: '89 分', status: 'accepted' },
    { id: 'cand-3', roomName: '像素工坊', title: '高能候选 #3：物理引擎穿模名场面', range: '00:54:10 - 00:56:05', score: '85 分', status: 'pending' },
  ];

  // 投稿示例数据
  const uploads = [
    { id: 'up-1', title: '【小岛电台】新地图开荒爆笑名场面合集', bvid: 'BV1sR4y1K7mP', account: '小岛切片组 (B 站)', status: '已通过审核', updated: '今天 12:30' },
    { id: 'up-2', title: '【像素工坊】从零手写 2D 光照系统实录', bvid: 'BV19x411c7Qa', account: '小岛切片组 (B 站)', status: '分 P 上传完成', updated: '昨天 23:40' },
  ];

  // 任务队列示例数据
  let tasks = [
    { id: 'task-1', type: '自动高能切片分析', target: '小岛电台 · 周五游戏夜', progress: 68, status: 'running', detail: '正在结合弹幕密度与语音字幕评分 (68%)' },
    { id: 'task-2', type: 'AI 录播摘要生成', target: '晚风聊天室 · 聊聊这周的新鲜事', progress: 45, status: 'running', detail: '正在调用 Whisper 转写时间轴字幕 (45%)' },
    { id: 'task-3', type: '整场切片导出 (含弹幕)', target: '像素工坊 · 独立游戏开发', progress: 100, status: 'done', detail: '已完成导出 (4.2 GB)' },
  ];

  // AI 助手对话示例数据
  const aiPresets = [
    { title: '分析高能片段', desc: '提取最近一场录播的弹幕峰值与精彩区间', prompt: '帮我分析最新一场小岛电台录播有哪些高能片段？' },
    { title: '检查录制状态', desc: '汇总当前所有在线与异常状态的直播间', prompt: '显示当前所有直播间的监测和录制状态' },
    { title: '批量生成摘要', desc: '为未生成摘要的录播创建 AI 章节总结', prompt: '为最近未处理的录播生成 AI 章节摘要' },
  ];

  let aiMessages = [
    { role: 'bot', text: '你好！我是 ShadowReplay 录播助手。你可以让我查询直播间状态、分析录播弹幕高能区间，或直接创建切片任务。' },
  ];

  // 账号示例数据
  let accounts = [
    { id: 'acc-1', name: '小岛切片组', uid: 'UID: 38192041', platform: 'B 站', status: '凭证有效（已加密存储）' },
    { id: 'acc-2', name: '晚风录播号', uid: 'SecUID: MS4wLjAB...', platform: '抖音', status: '凭证有效（已加密存储）' },
  ];

  /** 读取本地预览状态；存储不可用或内容无效时使用默认值。 */
  function readState() {
    try {
      const value = JSON.parse(localStorage.getItem('shadowreplay-prototype') || '{}');
      return {
        page: PAGE_TITLES[value?.page] ? value.page : 'summary',
        theme: ['auto', 'light', 'dark'].includes(value?.theme) ? value.theme : 'auto',
        density: value?.density === 'compact' ? 'compact' : 'comfortable',
        collapsed: Boolean(value?.collapsed),
      };
    } catch {
      return { page: 'summary', theme: 'auto', density: 'comfortable', collapsed: false };
    }
  }

  /** 保存页面和设计选项；浏览器禁用本地存储时继续使用内存状态。 */
  function remember() {
    try {
      localStorage.setItem('shadowreplay-prototype', JSON.stringify({ page: currentPage, ...design }));
    } catch {
      // 本地文件或隐私模式下可能无法写入存储，当前页面的交互不受影响。
    }
  }

  /** 同步文档与原型容器的主题、信息密度和侧边栏折叠状态。 */
  function applyDesign() {
    document.documentElement.style.colorScheme = design.theme === 'auto' ? 'light dark' : design.theme;
    root.dataset.theme = design.theme;
    root.dataset.density = design.density;
    root.dataset.collapsed = String(design.collapsed);
    const densityLabel = root.querySelector('#sr-density-label');
    if (densityLabel) {
      densityLabel.textContent = design.density === 'compact' ? '紧凑视图' : '舒适视图';
    }
  }

  /** 在所有已实现的原型页面之间切换。 */
  function showPage(page) {
    if (!PAGE_TITLES[page]) return;
    currentPage = page;
    Object.keys(PAGE_TITLES).forEach(key => {
      const section = root.querySelector(`#sr-${key}`);
      if (section) section.hidden = key !== page;
    });
    root.querySelector('#sr-current-page').textContent = PAGE_TITLES[page];
    root.querySelectorAll('.sr-nav [data-page]').forEach(button => {
      if (button.dataset.page === page) button.setAttribute('aria-current', 'page');
      else button.removeAttribute('aria-current');
    });
    root.querySelector('#sr-task-popover').hidden = true;
    root.querySelector('#sr-task-popover-btn').setAttribute('aria-expanded', 'false');
  }

  /** 显示预览操作结果提示。 */
  function toast(message) {
    const element = root.querySelector('#sr-toast');
    clearTimeout(toastTimer);
    element.textContent = message;
    element.hidden = false;
    toastTimer = setTimeout(() => { element.hidden = true; }, 3200);
  }

  /** 返回直播间检测状态对应的样式类与中文文案。 */
  function getStatusMeta(status) {
    switch (status) {
      case 'live':
        return { cls: '', label: '直播中' };
      case 'unknown':
        return { cls: 'sr-warn', label: '监测状态未知' };
      case 'unconfirmed':
        return { cls: 'sr-offline', label: '等待首次检测' };
      default:
        return { cls: 'sr-offline', label: '未直播' };
    }
  }

  /** 同步侧边栏角标、总览统计卡片与直播间副标题计数。 */
  function syncCountsAndSummary() {
    const liveRooms = rooms.filter(r => r.status === 'live');
    const recordingRooms = rooms.filter(r => r.status === 'live' && r.enabled);
    const offlineCount = rooms.filter(r => r.status === 'offline').length;
    const unknownCount = rooms.filter(r => r.status === 'unknown' || r.status === 'unconfirmed').length;
    const runningTasks = tasks.filter(t => t.status === 'running');

    root.querySelector('#sr-nav-room-count').textContent = String(rooms.length);
    root.querySelector('#sr-nav-archive-count').textContent = String(archives.length);
    root.querySelector('#sr-nav-material-count').textContent = String(materials.length);
    root.querySelector('#sr-nav-clip-count').textContent = String(clipProjects.length);
    root.querySelector('#sr-nav-task-count').textContent = String(runningTasks.length);
    root.querySelector('#sr-running-task-badge').textContent = String(runningTasks.length);
    root.querySelector('#sr-nav-account-count').textContent = String(accounts.length);
    root.querySelector('#sr-summary-account-count').textContent = String(accounts.length);

    root.querySelector('#sr-stat-online').textContent = String(liveRooms.length);
    root.querySelector('#sr-stat-total').textContent = String(rooms.length);
    root.querySelector('#sr-stat-recording-text').textContent = `正在缓存 ${recordingRooms.length} 个直播间`;
    root.querySelector('#sr-stat-today-records').textContent = String(archives.length);

    const extraStatus = unknownCount > 0 ? `<span class="sr-warn-text">，${unknownCount} 个状态未知</span>` : '';
    root.querySelector('#sr-rooms-subtitle').innerHTML =
      `<span class="sr-live">${liveRooms.length} 个直播中</span><span>，${offlineCount} 个未直播</span>${extraStatus}`;

    // 渲染总览页最近录播（前 3 条）
    root.querySelector('#sr-record-list').innerHTML = archives.slice(0, 3).map(rec => `
      <article class="sr-record">
        <div class="sr-thumbnail" aria-label="录播封面占位"><i data-lucide="film" aria-hidden="true"></i></div>
        <div>
          <h3>${rec.title}</h3>
          <div class="sr-meta">
            <span>${rec.roomName}</span>
            <span>${rec.platform}</span>
            <span>${rec.createdText}</span>
            ${rec.summaryStatus === 'success' ? '<span class="sr-tag sr-tag-green">AI 摘要已就绪</span>' : ''}
          </div>
          <div class="sr-meta"><span>${rec.durationText}</span><span>${rec.sizeText}</span></div>
        </div>
        <button type="button" class="sr-play" data-preview-record="${rec.title}" aria-label="回看 ${rec.title}" title="打开播放器回看">
          <i data-lucide="play" aria-hidden="true"></i>
        </button>
      </article>`).join('');

    // 渲染总览页直播中的房间快捷卡片
    root.querySelector('#sr-live-room-summary').innerHTML = liveRooms.map(room => `
      <button type="button" class="sr-mini-room" data-jump-room="${room.id}">
        <span class="sr-avatar">${room.avatar}</span>
        <div>
          <div>${room.name}</div>
          <div class="sr-caption">${room.platform} · ${room.time}</div>
        </div>
        <span class="sr-dot sr-live" aria-label="直播中"></span>
      </button>`).join('');

    // 渲染顶部任务中心浮层
    root.querySelector('#sr-popover-task-list').innerHTML = runningTasks.length
      ? runningTasks.map(task => `
        <div class="sr-popover-item">
          <div class="sr-inline" style="justify-content: space-between">
            <strong>${task.type}</strong>
            <span class="sr-caption">${task.progress}%</span>
          </div>
          <div class="sr-caption" style="margin-top: 3px">${task.target}</div>
          <div class="sr-meter" style="margin: 8px 0 0"><span style="width: ${task.progress}%"></span></div>
        </div>`).join('')
      : '<div class="sr-caption">当前没有正在执行的后台任务。</div>';
  }

  /** 根据平台和搜索词过滤直播间卡片。 */
  function filterRooms() {
    const query = root.querySelector('#sr-search').value.trim().toLowerCase();
    let visible = 0;
    root.querySelectorAll('.sr-room-card').forEach((card, index) => {
      const room = rooms[index];
      if (!room) return;
      const matchesPlatform = platform === '全部' || room.platform === platform;
      const matchesQuery = (room.name + room.title + room.id).toLowerCase().includes(query);
      card.hidden = !(matchesPlatform && matchesQuery);
      if (!card.hidden) visible++;
    });
    root.querySelector('#sr-no-match').hidden = visible > 0;
  }

  /** 渲染直播间卡片，包含房间号水印、4 态检测徽章、右上角更多菜单与自动切片标识。 */
  function renderRooms() {
    root.querySelector('#sr-room-grid').innerHTML = rooms.map((room, index) => {
      const statusMeta = getStatusMeta(room.status);
      const isRecording = room.status === 'live' && room.enabled;
      const menuOpen = openMenuRoomId === room.id;
      return `
      <article class="sr-room-card">
        <div class="sr-room-media">
          <span class="sr-state-badge ${statusMeta.cls}" ${room.errorHint ? `title="${room.errorHint}"` : ''}>
            <span class="sr-dot"></span>${statusMeta.label}
          </span>
          <button type="button" class="sr-card-more" data-room-menu="${room.id}" aria-label="${room.name}更多操作" title="更多操作">
            <i data-lucide="ellipsis" aria-hidden="true"></i>
          </button>
          ${menuOpen ? `
            <div class="sr-card-menu" role="menu">
              <button type="button" role="menuitem" data-open-auto="${room.id}"><i data-lucide="sparkles" aria-hidden="true"></i>自动切片设置</button>
              <button type="button" role="menuitem" data-open-web="${room.name}"><i data-lucide="external-link" aria-hidden="true"></i>打开网页直播间</button>
              <button type="button" role="menuitem" class="sr-danger-item" data-delete-room="${room.id}"><i data-lucide="trash-2" aria-hidden="true"></i>移除直播间</button>
            </div>
          ` : ''}
          <i data-lucide="${room.status === 'live' ? 'video' : 'video-off'}" aria-hidden="true"></i>
          <span>${room.status === 'live' ? '直播画面实时缓存中' : room.status === 'unknown' ? '监测请求受限，等待自动重试' : '等待下一次开播'}</span>
          <span class="sr-watermark">${room.code}#${room.id}</span>
          ${isRecording ? '<span class="sr-rec-badge"><span class="sr-dot"></span>录制中</span>' : ''}
        </div>
        <div class="sr-room-body">
          <h2 class="sr-room-title">
            <span>${room.title}</span>
            ${room.autoClip ? '<span class="sr-tag"><i data-lucide="sparkles" aria-hidden="true"></i>自动切片</span>' : ''}
          </h2>
          <div class="sr-room-author">
            <span class="sr-avatar">${room.avatar}</span>
            <span>${room.name}</span>
            <span class="sr-platform-label">${room.platform}</span>
          </div>
          <div class="sr-caption" style="margin-top: 9px">${room.time}</div>
          <div class="sr-room-bottom">
            <div class="sr-room-links">
              <button type="button" class="sr-text-button" data-preview-record="${room.title}">预览</button>
              <button type="button" class="sr-text-button" data-room-archives="${room.id}">录播</button>
              <button type="button" class="sr-text-button" data-open-auto="${room.id}">智能切片</button>
            </div>
            <button type="button" class="sr-switch" role="switch" aria-checked="${room.enabled}" aria-label="${room.name}的自动录制" data-toggle="${index}">
              <span>自动录制</span>
              <span class="sr-switch-track" aria-hidden="true"></span>
            </button>
          </div>
        </div>
      </article>`;
    }).join('');
    filterRooms();
  }

  /** 渲染录播归档页面与房间筛选下拉框。 */
  function renderArchives() {
    const filterSelect = root.querySelector('#sr-archive-room-filter');
    const selectedRoom = filterSelect.value || 'all';
    const sortBy = root.querySelector('#sr-archive-sort').value || 'time';

    filterSelect.innerHTML = '<option value="all">全部直播间</option>' +
      rooms.map(r => `<option value="${r.id}" ${selectedRoom === r.id ? 'selected' : ''}>${r.name} (${r.id})</option>`).join('');

    const list = archives
      .filter(a => selectedRoom === 'all' || a.roomId === selectedRoom)
      .slice()
      .sort((a, b) => {
        if (sortBy === 'size') return b.sizeGb - a.sizeGb;
        if (sortBy === 'duration') return b.durationSec - a.durationSec;
        return b.createdMs - a.createdMs;
      });

    root.querySelector('#sr-archive-table').innerHTML = list.map(item => `
      <article class="sr-record">
        <div class="sr-thumbnail"><i data-lucide="film" aria-hidden="true"></i></div>
        <div>
          <h3>${item.title}</h3>
          <div class="sr-meta">
            <span>${item.roomName} (#${item.roomId})</span>
            <span>${item.platform}</span>
            <span>${item.createdText}</span>
            <span>时长 ${item.durationText}</span>
            <span>大小 ${item.sizeText}</span>
          </div>
        </div>
        <div class="sr-inline">
          <button type="button" class="sr-button" data-summary-record="${item.id}">
            <i data-lucide="sparkles" aria-hidden="true"></i>
            ${item.summaryStatus === 'success' ? '查看 AI 摘要' : item.summaryStatus === 'processing' ? '摘要生成中' : '生成 AI 摘要'}
          </button>
          <button type="button" class="sr-button" data-whole-clip="${item.title}">
            <i data-lucide="clapperboard" aria-hidden="true"></i>生成整场切片
          </button>
          <button type="button" class="sr-play" data-preview-record="${item.title}" title="回看并打标切片">
            <i data-lucide="play" aria-hidden="true"></i>
          </button>
        </div>
      </article>`).join('');
  }

  /** 渲染素材、切片工作台、投稿、任务、AI 助手与账号页。 */
  function renderSecondaryPages() {
    // 素材页
    root.querySelector('#sr-material-list').innerHTML = materials.map(m => `
      <article class="sr-record">
        <div class="sr-thumbnail"><i data-lucide="film" aria-hidden="true"></i></div>
        <div>
          <h3>${m.title}</h3>
          <div class="sr-meta"><span>来源：${m.roomName}</span><span>${m.duration}</span><span>${m.size}</span><span>${m.created}</span></div>
        </div>
        <div class="sr-inline">
          <button type="button" class="sr-button" data-preview-record="${m.title}"><i data-lucide="play" aria-hidden="true"></i>预览</button>
          <button type="button" class="sr-button sr-button-primary" data-page="clips">加入切片工程</button>
        </div>
      </article>`).join('');

    // 切片工程视图
    root.querySelector('#sr-clip-projects-view').innerHTML = clipProjects.map(p => `
      <article class="sr-storage">
        <div class="sr-section-head">
          <h2>${p.title}</h2>
          <span class="sr-tag">${p.clips} 个片段</span>
        </div>
        <div class="sr-meta"><span>总时长 ${p.duration}</span><span>更新于 ${p.updated}</span></div>
        <div class="sr-caption" style="margin-top: 8px">${p.note}</div>
        <div class="sr-dialog-actions" style="margin-top: 14px">
          <button type="button" class="sr-button" data-page="uploads"><i data-lucide="upload" aria-hidden="true"></i>去投稿</button>
          <button type="button" class="sr-button sr-button-primary" data-preview-record="${p.title}">打开非线性编辑器</button>
        </div>
      </article>`).join('');

    // 自动切片候选视图
    root.querySelector('#sr-clip-automation-view').innerHTML = autoCandidates.map(c => `
      <article class="sr-record">
        <div class="sr-thumbnail"><i data-lucide="sparkles" aria-hidden="true"></i></div>
        <div>
          <h3>${c.title}</h3>
          <div class="sr-meta">
            <span>${c.roomName}</span>
            <span>区间 ${c.range}</span>
            <span class="sr-tag">${c.score}</span>
            ${c.status === 'accepted' ? '<span class="sr-tag sr-tag-green">已转入切片工程</span>' : '<span class="sr-tag sr-tag-warn">待审核</span>'}
          </div>
        </div>
        <div class="sr-inline">
          ${c.status === 'pending'
            ? `<button type="button" class="sr-button sr-button-primary" data-accept-candidate="${c.id}">采用并创建切片</button>`
            : `<button type="button" class="sr-button" disabled>已采用</button>`}
        </div>
      </article>`).join('');

    // 投稿页
    root.querySelector('#sr-upload-list').innerHTML = uploads.map(u => `
      <article class="sr-record">
        <div class="sr-thumbnail"><i data-lucide="upload" aria-hidden="true"></i></div>
        <div>
          <h3>${u.title}</h3>
          <div class="sr-meta"><span>${u.bvid}</span><span>账号：${u.account}</span><span class="sr-tag sr-tag-green">${u.status}</span><span>${u.updated}</span></div>
        </div>
        <button type="button" class="sr-button" data-open-web="${u.bvid}"><i data-lucide="external-link" aria-hidden="true"></i>查看稿件</button>
      </article>`).join('');

    // 任务页
    root.querySelector('#sr-task-list').innerHTML = tasks.map(t => `
      <article class="sr-record">
        <div class="sr-thumbnail"><i data-lucide="list-checks" aria-hidden="true"></i></div>
        <div>
          <h3>${t.type} · ${t.target}</h3>
          <div class="sr-meta">
            <span class="sr-tag ${t.status === 'done' ? 'sr-tag-green' : ''}">${t.status === 'done' ? '已完成' : '执行中'}</span>
            <span>${t.detail}</span>
          </div>
          <div class="sr-meter" style="margin: 8px 0 0"><span style="width: ${t.progress}%"></span></div>
        </div>
        <span class="sr-caption">${t.progress}%</span>
      </article>`).join('');

    // AI 助手页预设与消息
    root.querySelector('#sr-ai-presets').innerHTML = aiPresets.map(p => `
      <button type="button" class="sr-ai-preset" data-ai-prompt="${p.prompt}">
        <strong>${p.title}</strong>
        <span class="sr-caption">${p.desc}</span>
      </button>`).join('');

    root.querySelector('#sr-ai-chat').innerHTML = aiMessages.map(m => `
      <div class="sr-ai-msg ${m.role}">${m.text}</div>`).join('');

    // 账号页
    root.querySelector('#sr-account-list').innerHTML = accounts.map(a => `
      <article class="sr-record">
        <div class="sr-avatar">${a.name.slice(0, 1)}</div>
        <div>
          <h3>${a.name}</h3>
          <div class="sr-meta"><span>${a.platform}</span><span>${a.uid}</span><span class="sr-tag sr-tag-green">${a.status}</span></div>
        </div>
        <button type="button" class="sr-button" data-verify-account="${a.name}">检测状态</button>
      </article>`).join('');
  }

  /** 使用随原型提供的 Lucide 图标数据替换所有占位节点。 */
  function renderIcons() {
    const namespace = 'http://www.w3.org/2000/svg';
    root.querySelectorAll('[data-lucide]').forEach(placeholder => {
      const nodes = SHADOWREPLAY_ICONS[placeholder.dataset.lucide];
      if (!nodes) return;
      const svg = document.createElementNS(namespace, 'svg');
      const attributes = {
        viewBox: '0 0 24 24', width: '16', height: '16', fill: 'none',
        stroke: 'currentColor', 'stroke-width': '2',
        'stroke-linecap': 'round', 'stroke-linejoin': 'round',
        'aria-hidden': 'true', focusable: 'false',
      };
      Object.entries(attributes).forEach(([name, value]) => svg.setAttribute(name, value));
      nodes.forEach(([tag, properties]) => {
        const node = document.createElementNS(namespace, tag);
        Object.entries(properties).forEach(([name, value]) => node.setAttribute(name, value));
        svg.append(node);
      });
      placeholder.replaceWith(svg);
    });
  }

  /** 刷新全部视图并重新渲染图标。 */
  function refreshAllViews() {
    syncCountsAndSummary();
    renderRooms();
    renderArchives();
    renderSecondaryPages();
    renderIcons();
  }

  /** 关闭添加直播间弹窗并还原焦点。 */
  function closeDialog() {
    root.querySelector('#sr-add-dialog').hidden = true;
    dialogTrigger?.focus();
  }

  /** 关闭自动切片设置弹窗并还原焦点。 */
  function closeAutoDialog() {
    root.querySelector('#sr-automation-dialog').hidden = true;
    editingAutoRoomId = null;
    dialogTrigger?.focus();
  }

  root.addEventListener('click', event => {
    const button = event.target.closest('button');
    if (!button) {
      if (openMenuRoomId !== null) {
        openMenuRoomId = null;
        renderRooms();
        renderIcons();
      }
      return;
    }

    // 1. 切换页面
    if (button.dataset.page) {
      showPage(button.dataset.page);
      remember();
      return;
    }

    // 2. 直播间卡片右上角更多菜单开关
    if (button.hasAttribute('data-room-menu')) {
      const roomId = button.dataset.roomMenu;
      openMenuRoomId = openMenuRoomId === roomId ? null : roomId;
      renderRooms();
      renderIcons();
      return;
    }

    // 点击其他按钮时收起已打开的卡片菜单
    if (openMenuRoomId !== null) {
      openMenuRoomId = null;
    }

    // 3. 平台筛选
    if (button.hasAttribute('data-platform')) {
      platform = button.dataset.platform;
      root.querySelectorAll('[data-platform]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
      filterRooms();
      return;
    }

    // 4. 切片页子 Tab 切换
    if (button.hasAttribute('data-clip-tab')) {
      const tab = button.dataset.clipTab;
      root.querySelectorAll('[data-clip-tab]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
      root.querySelector('#sr-clip-projects-view').hidden = tab !== 'projects';
      root.querySelector('#sr-clip-automation-view').hidden = tab !== 'automation';
      return;
    }

    // 5. 自动录制开关切换
    if (button.hasAttribute('data-toggle')) {
      const room = rooms[Number(button.dataset.toggle)];
      if (!room) return;
      room.enabled = !room.enabled;
      room.recording = room.status === 'live' && room.enabled;
      room.time = room.enabled
        ? (room.status === 'live' ? '已恢复实时缓存' : '等待开播')
        : '自动录制已关闭';
      refreshAllViews();
      toast(`已${room.enabled ? '开启' : '关闭'}「${room.name}」的自动录制`);
      return;
    }

    // 6. 总览页快捷卡片跳转到指定直播间
    if (button.hasAttribute('data-jump-room')) {
      showPage('rooms');
      remember();
      toast('已定位到直播间列表');
      return;
    }

    // 7. 从直播间卡片跳转到对应房间的录播归档
    if (button.hasAttribute('data-room-archives')) {
      const roomId = button.dataset.roomArchives;
      root.querySelector('#sr-archive-room-filter').value = roomId;
      renderArchives();
      renderIcons();
      showPage('archives');
      remember();
      return;
    }

    // 8. 打开自动切片设置弹窗
    if (button.hasAttribute('data-open-auto')) {
      const room = rooms.find(r => r.id === button.dataset.openAuto);
      if (!room) return;
      dialogTrigger = button;
      editingAutoRoomId = room.id;
      root.querySelector('#sr-auto-room-subtitle').textContent = `配置「${room.name} (${room.code}#${room.id})」的下播自动高能切片规则。`;
      root.querySelector('#sr-auto-enable-switch').setAttribute('aria-checked', String(room.autoClip));
      root.querySelector('#sr-auto-threshold').value = String(room.threshold);
      root.querySelector('#sr-auto-keywords').value = room.keywords;
      root.querySelector('#sr-automation-dialog').hidden = false;
      renderRooms();
      renderIcons();
      return;
    }

    // 9. 移除直播间
    if (button.hasAttribute('data-delete-room')) {
      const roomId = button.dataset.deleteRoom;
      const target = rooms.find(r => r.id === roomId);
      rooms = rooms.filter(r => r.id !== roomId);
      refreshAllViews();
      toast(`已移除直播间「${target?.name || roomId}」`);
      return;
    }

    // 10. 采用自动切片候选
    if (button.hasAttribute('data-accept-candidate')) {
      const cand = autoCandidates.find(c => c.id === button.dataset.acceptCandidate);
      if (cand) {
        cand.status = 'accepted';
        refreshAllViews();
        toast(`已将「${cand.title}」加入切片工程`);
      }
      return;
    }

    // 11. 预览回看 / AI 摘要 / 整场切片 / 外部网页
    if (button.hasAttribute('data-preview-record')) {
      toast(`示例预览：正在打开播放器窗口「${button.dataset.previewRecord}」`);
      return;
    }
    if (button.hasAttribute('data-summary-record')) {
      const rec = archives.find(a => a.id === button.dataset.summaryRecord);
      if (rec) {
        rec.summaryStatus = 'success';
        refreshAllViews();
        toast(`「${rec.title}」AI 章节摘要已就绪`);
      }
      return;
    }
    if (button.hasAttribute('data-whole-clip')) {
      toast(`已加入任务队列：整场切片「${button.dataset.wholeClip}」`);
      return;
    }
    if (button.hasAttribute('data-open-web')) {
      toast(`示例：已在外部浏览器打开「${button.dataset.openWeb}」`);
      return;
    }
    if (button.hasAttribute('data-verify-account')) {
      toast(`账号「${button.dataset.verifyAccount}」Cookie 状态有效`);
      return;
    }

    // 12. AI 预设提示词点击
    if (button.hasAttribute('data-ai-prompt')) {
      const promptText = button.dataset.aiPrompt;
      aiMessages.push({ role: 'user', text: promptText });
      aiMessages.push({
        role: 'bot',
        text: `已执行指令：「${promptText}」。共检测到 2 个在线直播间（小岛电台、晚风聊天室），并在最新录播中识别出 3 段弹幕峰值高能区间。`,
      });
      renderSecondaryPages();
      return;
    }

    // 13. 打开/关闭添加直播间弹窗
    if (button.hasAttribute('data-add')) {
      dialogTrigger = button;
      root.querySelector('#sr-add-dialog').hidden = false;
      root.querySelector('#sr-add-room-id').focus();
      return;
    }
    if (button.hasAttribute('data-close')) {
      closeDialog();
      return;
    }
    if (button.hasAttribute('data-close-auto')) {
      closeAutoDialog();
    }
  });

  // 侧边栏折叠切换
  root.querySelector('#sr-sidebar-toggle').addEventListener('click', () => {
    design.collapsed = !design.collapsed;
    applyDesign();
    remember();
  });

  // 信息密度切换（舒适 / 紧凑）
  root.querySelector('#sr-density').addEventListener('click', () => {
    design.density = design.density === 'compact' ? 'comfortable' : 'compact';
    applyDesign();
    remember();
  });

  // 深浅主题切换
  root.querySelector('#sr-theme').addEventListener('click', () => {
    const isDark = design.theme === 'dark' || (design.theme === 'auto' && matchMedia('(prefers-color-scheme: dark)').matches);
    design.theme = isDark ? 'light' : 'dark';
    applyDesign();
    remember();
  });

  // 顶部任务中心浮层切换
  root.querySelector('#sr-task-popover-btn').addEventListener('click', event => {
    event.stopPropagation();
    const popover = root.querySelector('#sr-task-popover');
    const nextHidden = !popover.hidden;
    popover.hidden = nextHidden;
    event.currentTarget.setAttribute('aria-expanded', String(!nextHidden));
  });

  // 搜索与录播筛选事件
  root.querySelector('#sr-search').addEventListener('input', filterRooms);
  root.querySelector('#sr-archive-room-filter').addEventListener('change', () => { renderArchives(); renderIcons(); });
  root.querySelector('#sr-archive-sort').addEventListener('change', () => { renderArchives(); renderIcons(); });
  root.querySelector('#sr-refresh').addEventListener('click', () => toast('最近录播列表已同步最新状态'));

  // 添加直播间提交
  root.querySelector('#sr-add-confirm').addEventListener('click', () => {
    const selectedPlat = root.querySelector('#sr-add-platform').value;
    const rawId = root.querySelector('#sr-add-room-id').value.trim() || String(Math.floor(100000 + Math.random() * 900000));
    const rawName = root.querySelector('#sr-add-room-name').value.trim() || `${selectedPlat}新主播`;
    const codeMap = { 'B 站': 'BILIBILI', '抖音': 'DOUYIN', '斗鱼': 'DOUYU', '虎牙': 'HUYA', '快手': 'KUAISHOU', 'TikTok': 'TIKTOK', 'Twitch': 'TWITCH', 'YouTube': 'YOUTUBE' };
    rooms.unshift({
      id: rawId.replace(/^.*\/|\?.*$/g, '') || rawId,
      code: codeMap[selectedPlat] || 'LIVE',
      name: rawName,
      avatar: rawName.slice(0, 1),
      title: `${rawName}的直播间`,
      platform: selectedPlat,
      status: 'unconfirmed',
      recording: false,
      enabled: true,
      autoClip: false,
      threshold: 75,
      keywords: '666, 哈哈',
      time: '等待首次检测',
      errorHint: '',
    });
    root.querySelector('#sr-add-room-id').value = '';
    root.querySelector('#sr-add-room-name').value = '';
    closeDialog();
    refreshAllViews();
    showPage('rooms');
    toast(`已添加直播间「${rawName}」`);
  });

  // 自动切片弹窗内开关与保存
  root.querySelector('#sr-auto-enable-switch').addEventListener('click', event => {
    const btn = event.currentTarget;
    const checked = btn.getAttribute('aria-checked') === 'true';
    btn.setAttribute('aria-checked', String(!checked));
  });

  root.querySelector('#sr-auto-save').addEventListener('click', () => {
    const room = rooms.find(r => r.id === editingAutoRoomId);
    if (room) {
      room.autoClip = root.querySelector('#sr-auto-enable-switch').getAttribute('aria-checked') === 'true';
      room.threshold = Number(root.querySelector('#sr-auto-threshold').value) || 75;
      room.keywords = root.querySelector('#sr-auto-keywords').value.trim();
      refreshAllViews();
      toast(`已保存「${room.name}」的自动切片配置`);
    }
    closeAutoDialog();
  });

  // AI 助手表单发送
  root.querySelector('#sr-ai-form').addEventListener('submit', event => {
    event.preventDefault();
    const input = root.querySelector('#sr-ai-input');
    const text = input.value.trim();
    if (!text) return;
    aiMessages.push({ role: 'user', text });
    aiMessages.push({
      role: 'bot',
      text: `收到指令：「${text}」。在原型预览模式下，系统已完成模拟分析并更新候选结果。`,
    });
    input.value = '';
    renderSecondaryPages();
  });

  // 次级页面快捷操作提示
  root.querySelector('#sr-import-material').addEventListener('click', () => toast('示例：选择本地 MP4/MKV 视频导入素材库'));
  root.querySelector('#sr-create-clip-from-material').addEventListener('click', () => { showPage('clips'); toast('已进入切片工作台'); });
  root.querySelector('#sr-new-upload').addEventListener('click', () => toast('示例：请选择切片工程或素材发起 B 站投稿'));
  root.querySelector('#sr-clear-done-tasks').addEventListener('click', () => {
    tasks = tasks.filter(t => t.status !== 'done');
    refreshAllViews();
    toast('已清理完成的后台任务');
  });
  root.querySelector('#sr-add-account').addEventListener('click', () => toast('示例：支持 B 站扫码登录或粘贴平台 Cookie'));
  root.querySelector('#sr-check-update').addEventListener('click', () => toast('当前已是最新版本 ShadowReplay 2.24.1'));

  // 弹窗键盘 Escape 与 Tab 焦点环控制
  ['#sr-add-dialog', '#sr-automation-dialog'].forEach(selector => {
    root.querySelector(selector).addEventListener('keydown', event => {
      if (event.key === 'Escape') {
        if (selector === '#sr-add-dialog') closeDialog();
        else closeAutoDialog();
        return;
      }
      if (event.key !== 'Tab') return;
      const controls = [...root.querySelectorAll(`${selector} button, ${selector} input, ${selector} select`)];
      if (event.shiftKey && document.activeElement === controls[0]) { event.preventDefault(); controls.at(-1).focus(); }
      else if (!event.shiftKey && document.activeElement === controls.at(-1)) { event.preventDefault(); controls[0].focus(); }
    });
  });

  applyDesign();
  refreshAllViews();
  showPage(currentPage);
})();
