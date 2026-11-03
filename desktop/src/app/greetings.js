/* app/greetings.js — the line above the prompt on the start page.
   Half the time it is the greeting of the hour (the same for Chat and Code:
   morning, noon, afternoon, evening, past midnight; Academic words it for
   study), otherwise one of the lines of the current mode. `{name}` is the user's name; without one the
   token goes, with its comma. `greeting({ mode, name, hour, seed })` is pure:
   `seed` in [0, 1) picks the line. Regression: test-greetings.mjs. */
(function () {
  const PERIODS = {
    morning: ["Good morning, {name}", "Rise and shine, {name}", "A fresh morning, {name}", "Morning, {name}. Coffee first?"],
    noon: ["Good noon, {name}", "Lunchtime, {name}?", "Midday already, {name}", "Halfway through the day, {name}"],
    afternoon: ["Good afternoon, {name}", "Afternoon, {name}", "Good to see you, {name}. The afternoon is young", "A quiet afternoon, {name}?"],
    evening: ["Good evening, {name}", "Evening, {name}", "Winding down, {name}?", "The day is nearly done, {name}"],
    midnight: ["Night owl! {name}", "Still up, {name}?", "Burning the midnight oil, {name}", "The world is asleep, {name}"],
  };

  // Academic's own take on the hour.
  const ACADEMIC_PERIODS = {
    morning: ["Good morning, {name}. A fresh day for reading", "Morning, {name}. Which paper first?", "A clear morning for deep work, {name}"],
    noon: ["Midday, {name}. Time to skim the literature?", "A break between papers, {name}?", "Halfway through the reading list, {name}"],
    afternoon: ["Good afternoon, {name}. Back to the notes?", "An afternoon for close reading, {name}", "Afternoon, {name}. Where did we leave the draft?"],
    evening: ["Good evening, {name}. One more section?", "Evening, {name}. Time to sort the day's findings", "A quiet evening for reading, {name}"],
    midnight: ["Deadline night, {name}?", "Still reading, {name}? Don't forget to sleep", "Late-night lab hours, {name}"],
  };

  const BY_MODE = {
    chat: [
      "What is on your mind, {name}?", "How can I help, {name}?", "Ask me anything, {name}", "What are you curious about, {name}?",
      "Let's think it through, {name}", "Something to figure out, {name}?", "A question for me, {name}?", "What shall we talk about, {name}?",
      "Need a second opinion, {name}?", "What would you like to write or learn, {name}?", "Tell me the story, {name}", "Where shall we begin, {name}?",
    ],
    code: [
      "What shall we build, {name}?", "Which bug are we hunting, {name}?", "Ready to ship something, {name}?", "Pick a project, {name}",
      "What are we refactoring, {name}?", "What needs fixing, {name}?", "Show me the repo, {name}", "Tests first, {name}?",
      "What is the plan, {name}?", "Let's make it work, {name}", "Which file first, {name}?", "Open a folder and let's begin, {name}",
    ],
    academic: [
      "What are we researching, {name}?", "Which paper shall we read, {name}?", "What is your research question, {name}?", "Let's trace the evidence, {name}",
      "Where is the gap in the literature, {name}?", "Ready to survey the field, {name}?", "What does the data say, {name}?", "Let's check the citations, {name}",
      "Which hypothesis shall we test, {name}?", "Bring me a paper and let's begin, {name}", "What shall we compare, {name}?", "Let's build the argument, {name}",
    ],
  };

  // The other interface languages (app/i18n.js): the hour's greeting and the mode's own lines.
  // The name comes last, after a comma, so it can drop out; ja adds さん.
  const LOCALIZED = {
    "zh-CN": {
      periods: {
        morning: ["早上好，{name}", "新的一天开始了，{name}", "早安，{name}。先来杯咖啡？"],
        noon: ["中午好，{name}", "该吃午饭了吧，{name}？", "今天已经过半了，{name}"],
        afternoon: ["下午好，{name}", "午后时光，{name}", "下午安静吗，{name}？"],
        evening: ["晚上好，{name}", "一天快结束了，{name}", "今晚轻松一点，{name}？"],
        midnight: ["夜猫子，{name}！", "还没睡，{name}？", "夜深了，{name}"],
      },
      chat: ["有什么想聊的，{name}？", "我能帮上什么，{name}？", "随便问我，{name}", "今天好奇什么，{name}？", "一起想想看，{name}", "想写点什么或学点什么，{name}？"],
      code: ["我们要构建什么，{name}？", "今天追哪个 bug，{name}？", "准备发布点什么，{name}？", "选一个项目吧，{name}", "先写测试，{name}？", "打开文件夹，开始吧，{name}"],
      academic: ["我们研究什么，{name}？", "读哪篇论文，{name}？", "你的研究问题是什么，{name}？", "一起追溯证据，{name}", "文献里的空白在哪，{name}？", "带一篇论文来吧，{name}"],
    },
    "zh-TW": {
      periods: {
        morning: ["早安，{name}", "新的一天開始了，{name}", "早安，{name}。先來杯咖啡？"],
        noon: ["午安，{name}", "該吃午餐了吧，{name}？", "今天已經過半了，{name}"],
        afternoon: ["午安，{name}", "午後時光，{name}", "下午還安靜嗎，{name}？"],
        evening: ["晚安，{name}", "一天快結束了，{name}", "今晚輕鬆一點，{name}？"],
        midnight: ["夜貓子，{name}！", "還沒睡，{name}？", "夜深了，{name}"],
      },
      chat: ["有什麼想聊的，{name}？", "我能幫上什麼，{name}？", "盡管問我，{name}", "今天好奇什麼，{name}？", "一起想想看，{name}", "想寫點什麼或學點什麼，{name}？"],
      code: ["我們要建構什麼，{name}？", "今天追哪個 bug，{name}？", "準備發布點什麼，{name}？", "選一個專案吧，{name}", "先寫測試，{name}？", "開啟資料夾，開始吧，{name}"],
      academic: ["我們研究什麼，{name}？", "讀哪篇論文，{name}？", "你的研究問題是什麼，{name}？", "一起追溯證據，{name}", "文獻裡的缺口在哪，{name}？", "帶一篇論文來吧，{name}"],
    },
    "ja-JP": {
      periods: {
        morning: ["おはようございます、{name}さん", "新しい朝ですね、{name}さん", "おはようございます、{name}さん。まずコーヒーでも？"],
        noon: ["こんにちは、{name}さん", "お昼ですね、{name}さん", "今日ももう半分、{name}さん"],
        afternoon: ["こんにちは、{name}さん", "午後のひととき、{name}さん", "静かな午後ですね、{name}さん"],
        evening: ["こんばんは、{name}さん", "今日もお疲れさま、{name}さん", "ゆっくりしましょう、{name}さん"],
        midnight: ["夜更かしですね、{name}さん", "まだ起きていますか、{name}さん", "夜も更けました、{name}さん"],
      },
      chat: ["何について話しましょうか、{name}さん", "お手伝いできることは、{name}さん", "何でも聞いてください、{name}さん", "気になることは、{name}さん", "一緒に考えましょう、{name}さん", "何を書きますか、{name}さん"],
      code: ["何を作りましょうか、{name}さん", "今日はどのバグを追いますか、{name}さん", "何をリリースしますか、{name}さん", "プロジェクトを選びましょう、{name}さん", "まずテストから、{name}さん", "フォルダを開いて始めましょう、{name}さん"],
      academic: ["何を研究しますか、{name}さん", "どの論文を読みますか、{name}さん", "研究の問いは何ですか、{name}さん", "根拠をたどりましょう、{name}さん", "先行研究の空白はどこでしょう、{name}さん", "論文を持ってきてください、{name}さん"],
    },
  };

  /** The period of an hour (0-23): 5-10 morning, 11-13 noon, 14-17 afternoon, 18-22 evening, else midnight. */
  function periodOf(hour) {
    const h = ((Math.floor(Number(hour)) % 24) + 24) % 24;
    if (h >= 5 && h < 11) return "morning";
    if (h >= 11 && h < 14) return "noon";
    if (h >= 14 && h < 18) return "afternoon";
    if (h >= 18 && h < 23) return "evening";
    return "midnight";
  }

  /** A template with the name filled in, or without it: `, {name}` and ` {name}` go. */
  function fill(template, name) {
    const n = String(name ?? "").trim();
    if (n) return template.replace(/\{name\}/g, n);
    return template.replace(/[,，、]\s*\{name\}(さん)?/g, "").replace(/\s*\{name\}/g, "").trim();
  }

  const pickFrom = (list, r) => list[Math.min(list.length - 1, Math.floor(r * list.length))];

  function greeting({ mode = "code", name = "", hour = 12, seed = 0, locale = "en-US" } = {}) {
    const s = Math.min(0.999999, Math.max(0, Number(seed) || 0));
    const shared = s < 0.5;
    // Two independent-looking draws from one seed.
    const r = shared ? s * 2 : (s - 0.5) * 2;
    const modeKey = mode === "chat" || mode === "academic" ? mode : "code";
    const loc = LOCALIZED[locale];
    if (loc) return fill(pickFrom(shared ? loc.periods[periodOf(hour)] : loc[modeKey], r), name);
    const periods = mode === "academic" ? ACADEMIC_PERIODS : PERIODS;
    const list = shared ? periods[periodOf(hour)] : BY_MODE[modeKey];
    return fill(pickFrom(list, r), name);
  }

  window.OMP_GREETINGS = { greeting, periodOf, fill, PERIODS, ACADEMIC_PERIODS, BY_MODE, LOCALIZED };
})();
