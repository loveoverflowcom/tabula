(function(){"use strict";
  let launching=false;
  const form=document.getElementById("simulator-form"),error=document.getElementById("simulator-error");
  form.addEventListener("submit",event=>{
    event.preventDefault(); if(launching)return;
    try{
      const config=TabulaLaunch.parse(new URLSearchParams({game:"werewolf",mode:"simulator",seats:document.getElementById("simulator-seats").value,theme:document.getElementById("simulator-theme").value,motion:"system",locale:"vi"}).toString()); // xtask-allow-game-id: ADR-0035 opt-in standalone leaf.
      launching=true;location.assign(`play.html?${TabulaLaunch.query(config)}`);
    }catch(reason){launching=false;error.hidden=false;error.textContent=reason instanceof Error?reason.message:String(reason);}
  });
  window.addEventListener("pageshow",()=>{launching=false;});
})();
