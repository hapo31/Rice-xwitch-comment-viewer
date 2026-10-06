import assert from 'node:assert/strict';
// Start Vite, then: PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs node scripts/check-modal-browser.mjs http://127.0.0.1:22545
// Only the Tauri backend is mocked; AppShell, routing, React and browser focus are real.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const origin = process.argv[2] || 'http://127.0.0.1:22545';
const browser = await chromium.launch();
try {
 const page=await browser.newPage(); page.setDefaultTimeout(10000);
 page.on('pageerror',error=>console.log('PAGEERROR',error.message));
 await page.route(url => url.pathname === '/src/main.tsx' && !url.searchParams.has('proof'), route=>route.fulfill({contentType:'text/javascript',body:`
 import {mockIPC,mockWindows} from '/node_modules/@tauri-apps/api/mocks.js';
 import {createDefaultAppSettings} from '/src/settings/model.ts';
 let settings=createDefaultAppSettings();
 window.modalProof={holdSave:false,releaseSave:null};
 mockWindows('main'); mockIPC((cmd,args)=>{
 switch(cmd){
 case 'settings_get':return structuredClone(settings);
 case 'settings_update': {
  const patch=args.patch;
  settings={...settings,...patch,speech:{...settings.speech,...patch.speech},twitch:{...settings.twitch,...patch.twitch}};
  return window.modalProof.holdSave ? new Promise(resolve=>{window.modalProof.releaseSave=()=>resolve(structuredClone(settings))}) : structuredClone(settings);
 }
 case 'settings_take_recovery_notice':case 'twitch_get_stored_auth':return null;
 case 'app_build_info':return {version:'0.2.3',isDev:true,launcher:{canRegisterApplications:true,canLaunchApplications:true}};
 case 'app_events_snapshot':return {revision:0,logs:[],twitchStatuses:[],emitErrors:[]};
 case 'speech_queue_reload':return {revision:0,status:{revision:0,status:'idle',adapterHealth:'connected',occurredAtMs:1},queue:{revision:0,phase:'idle',items:[],queuedCount:0,occurredAtMs:1}};
 case 'plugin:window|is_maximized':return false;
 case 'speech_health_probe':return 'disconnected';
 default:throw new Error('Unexpected browser proof IPC: '+cmd);
 }
 },{shouldMockEvents:true});
 await import('/src/main.tsx?proof');
 `}));
 await page.goto(origin+'/#/settings'); await page.getByRole('heading',{name:'Settings',exact:true}).waitFor();
 await page.locator('#bouyomi-port').fill('50002');
 const trigger=page.getByRole('link',{name:'Chat',exact:true});
 await trigger.click();
 const dialog=page.getByRole('dialog',{name:'未保存の変更があります'});
 await dialog.waitFor();
 assert.equal(await page.evaluate(()=>document.activeElement.textContent),'キャンセル');
 for(let i=0;i<8;i++){await page.keyboard.press('Tab');assert.equal(await page.evaluate(()=>!!document.activeElement.closest('[role=dialog]')),true);}
 for(let i=0;i<8;i++){await page.keyboard.press('Shift+Tab');assert.equal(await page.evaluate(()=>!!document.activeElement.closest('[role=dialog]')),true);}
 await page.locator('#bouyomi-port').evaluate(e=>e.focus());
 assert.equal(await page.evaluate(()=>!!document.activeElement.closest('[role=dialog]')),true);
 await page.mouse.click(20,140);assert.equal(await dialog.count(),1);
 await page.keyboard.press('Escape');await dialog.waitFor({state:'detached'});
 await page.waitForFunction(()=>document.activeElement?.getAttribute('aria-label')==='Chat');
 console.log('PASS actual AppShell: Tab/Shift+Tab cycles, background focus/click blocked, Escape closes, trigger restored');
 await page.evaluate(()=>{window.modalProof.holdSave=true;});
 await trigger.click(); await page.getByRole('button',{name:'保存して続ける',exact:true}).click();
 await page.getByRole('button',{name:'保存しています…',exact:true}).waitFor();
 assert.equal(await page.getByRole('button',{name:'保存しています…',exact:true}).isDisabled(),true);
 for(let i=0;i<5;i++){await page.keyboard.press('Tab');assert.equal(await page.evaluate(()=>!!document.activeElement.closest('[role=dialog]')),true);}
 await page.keyboard.press('Escape'); await dialog.waitFor({state:'detached'});
 await page.evaluate(()=>window.modalProof.releaseSave());
 await page.waitForFunction(()=>document.activeElement?.getAttribute('aria-label')==='Chat');
 assert.match(page.url(), /#\/settings$/);
 assert.equal(await page.getByRole('dialog').count(),0);
 console.log('PASS saving remains cancellable; late save completion does not navigate');
 await page.locator('#bouyomi-port').fill('50003');
 await page.evaluate(async ()=>{ const {emit}=await import('/node_modules/@tauri-apps/api/event.js'); await emit('tauri://close-requested'); await emit('tauri://close-requested'); });
 await dialog.waitFor(); assert.equal(await page.getByRole('dialog').count(),1);
 await page.keyboard.press('Escape');await dialog.waitFor({state:'detached'});
 console.log('PASS repeated OS close requests share one confirmation');
} finally { await browser.close(); }
