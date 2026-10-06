import {icon} from './icons.mjs';
import {CHESS_INITIAL,XIANGQI_INITIAL} from './data.mjs';

/** Original vector game illustrations and board presenters; intentionally no game authority. */
const PIECE_PATHS = {
 p:'<circle cx="30" cy="15" r="7"/><path d="M24 23h12l-3 15 7 8H20l7-8zM17 49h26v5H17z"/>',
 r:'<path d="M16 8h7v7h5V8h5v7h5V8h7v14l-7 5v16l6 5v6H16v-6l6-5V27l-6-5z"/>',
 n:'<path d="m19 46 6-12-9-1-4-8L26 8l13-4-1 10 6 8 4 24H19zM15 49h35v5H15z"/><circle cx="29" cy="20" r="2" fill="var(--piece-eye)"/><path d="m17 26 7 1" fill="none" stroke="var(--piece-eye)" stroke-width="2"/>',
 b:'<path d="M30 3c-4 7-13 12-13 20 0 7 5 10 9 12l-4 10h16l-4-10c4-2 9-5 9-12C43 15 34 10 30 3ZM17 48h26v6H17z"/><path d="m33 13-7 13" fill="none" stroke="var(--piece-eye)" stroke-width="2.5"/>',
 q:'<path d="m13 16 8 9 9-15 9 15 8-9-7 25H20zm7 29h20l5 9H15z"/><circle cx="12" cy="12" r="4"/><circle cx="30" cy="6" r="4"/><circle cx="48" cy="12" r="4"/>',
 k:'<path d="M27 2h6v7h6v5h-6v5h-6v-5h-6V9h6zM18 21h24l-6 15 3 9H21l3-9zM17 48h26v6H17z"/>'
};
export function chessPiece(code){if(!code)return '';const white=code===code.toUpperCase();return `<svg viewBox="0 0 60 60" class="chess-piece ${white?'white':'black'}" aria-hidden="true">${PIECE_PATHS[code.toLowerCase()]}</svg>`;}
export function chessBoard(state={}, compact=false){
 const board=state.chess || CHESS_INITIAL; const selected=state.selectedChess; const marks=state.chessTargets||[];
 const names={p:'Tốt',r:'Xe',n:'Mã',b:'Tượng',q:'Hậu',k:'Vua'};
 return `<div class="chess-frame ${compact?'compact-board':''}"><div class="chess-board" role="group" aria-label="Bàn cờ vua minh họa">${board.map((row,r)=>row.map((p,c)=>{
 const square=`${'abcdefgh'[c]}${8-r}`; const sel=selected?.[0]===r&&selected?.[1]===c;const hint=marks.some(t=>t[0]===r&&t[1]===c);
 return `<button class="square ${(r+c)%2?'dark-square':'light-square'} ${sel?'selected-square':''} ${hint?'target-square':''} ${r===4&&c===4?'last-square':''}" data-chess="${r},${c}" aria-label="${square}${p?': '+names[p.toLowerCase()]+' '+(p===p.toUpperCase()?'trắng':'đen'):': ô trống'}" ${compact?'tabindex="-1"':''}>${c===0?`<span class="rank">${8-r}</span>`:''}${r===7?`<span class="file">${'abcdefgh'[c]}</span>`:''}${chessPiece(p)}${hint?'<i class="legal-dot"></i>':''}</button>`;
 }).join('')).join('')}</div></div>`;
}
export function xiangqiBoard(state={},options={}){
 const pieces=state.xiangqi||XIANGQI_INITIAL; const sel=state.selectedXq;const compact=options.compact;
 let lines='';for(let y=0;y<10;y++)lines+=`<path d="M40 ${40+y*48}H424"/>`;for(let x=0;x<9;x++)lines+=x===0||x===8?`<path d="M${40+x*48} 40V472"/>`:`<path d="M${40+x*48} 40V232M${40+x*48} 280V472"/>`;
 lines+='<path d="m184 40 96 96m0-96-96 96m0 240 96 96m0-96-96 96"/>';
 const positions=[[1,2],[7,2],[0,3],[2,3],[4,3],[6,3],[8,3],[1,7],[7,7],[0,6],[2,6],[4,6],[6,6],[8,6]];
 let marks='';for(const [x,y]of positions){const a=40+x*48,b=40+y*48;for(const dx of [-1,1])for(const dy of [-1,1]){if((x===0&&dx===-1)||(x===8&&dx===1))continue;marks+=`<path d="M${a+dx*5} ${b+dy*12}v${-dy*7}h${dx*7}"/>`;}}
 const arrow=options.arrow||state.analysisArrow;
 const coords=Array.from({length:9},(_,x)=>`<text x="${40+x*48}" y="505" class="xq-coordinate">${9-x}</text>`).join('');
 return `<div class="xiangqi-frame ${compact?'compact-board':''}"><svg viewBox="0 0 464 520" class="xiangqi-board" role="group" aria-label="Bàn cờ tướng minh họa"><rect x="0" y="0" width="464" height="520" rx="18" fill="var(--xq-paper)"/><rect x="31" y="31" width="402" height="450" rx="2" fill="none" stroke="var(--xq-line)" stroke-width="1.6"/><g stroke="var(--xq-line)" stroke-width="1" fill="none">${lines}${marks}</g><text x="137" y="263" class="river-text">楚 河</text><text x="329" y="263" class="river-text">漢 界</text>${coords}${pieces.map(([x,y,label,team],i)=>`<g class="xq-piece ${team} ${sel===i?'xq-selected':''}" data-xq="${i}" role="button" tabindex="${compact?-1:0}" aria-label="Quân ${label} ${team==='red'?'đỏ':'đen'}, cột ${x+1}, hàng ${y+1}" transform="translate(${40+x*48} ${40+y*48})"><circle cy="2" r="21" class="piece-shadow"/><circle r="21" class="piece-disc"/><circle r="17.5" class="piece-ring"/><text y="1">${label}</text></g>`).join('')}${arrow?'<g class="analysis-arrow" stroke="var(--primary)" stroke-width="6" fill="none" stroke-linecap="round" opacity=".83"><path d="M376 456V387H326"/><path d="m338 376-12 11 12 11"/></g>':''}</svg></div>`;
}
export function caroBoard(state={}){
 const points=state.caro||[[6,5,'x'],[6,6,'o'],[7,6,'x'],[5,5,'o'],[8,7,'x'],[4,5,'o'],[5,7,'x'],[4,6,'o'],[8,6,'x']];
 return `<div class="caro-frame"><div class="caro-board" role="group" aria-label="Bàn Caro minh họa">${Array.from({length:169},(_,i)=>{const r=Math.floor(i/13),c=i%13,p=points.find(p=>p[0]===r&&p[1]===c);return `<button class="caro-cell ${p?'has-piece '+p[2]:''} ${p===points[points.length-1]?'last-caro':''}" data-caro="${r},${c}" aria-label="Ô hàng ${r+1}, cột ${c+1}${p?', '+p[2]:', trống'}">${p?(p[2]==='x'?'<span class="cross"></span>':'<span class="circle"></span>'):''}</button>`}).join('')}</div></div>`;
}
export function tileSvg(type=0,rotation=0){
 const roads=['<path d="M50 0V100"/>','<path d="M0 50h32q18 0 18 18v32"/>','<path d="M50 0v50h50M50 50v50"/>','<path d="M0 50h100M50 50v50"/>'];
 return `<svg viewBox="0 0 100 100" style="transform:rotate(${rotation}deg)" aria-hidden="true"><rect width="100" height="100" rx="5" fill="${type%2?'#bbceb0':'#cad5b6'}"/><path d="M8 13h8m-4-4v8M75 70h8m-4-4v8M22 79h6" stroke="#76906c" stroke-width="1.3" opacity=".55"/>${type%3===0?'<path d="M0 0h100v24l-15-5-12 14-16-6-15 13L26 25 0 28z" fill="#c79872"/><path d="M0 8h100" stroke="#ead0af" stroke-width="3"/>':''}<g stroke="#f8ecd1" stroke-width="13" fill="none">${roads[type%4]}</g><g stroke="#c8b58e" stroke-width="1" fill="none">${roads[type%4]}</g>${type%4===1?'<path d="M19 22 30 10l11 12v17H19z" fill="#eff0d9"/><path d="m16 23 14-16 14 16" stroke="#995c4c" stroke-width="5" fill="none"/>':''}<circle cx="77" cy="23" r="9" fill="#6f926c"/><circle cx="70" cy="28" r="7" fill="#88a67b"/></svg>`;
}
export function tilesBoard(state={}){
 const cells=[[-1,-1,1,0],[0,-1,0,90],[1,-1,2,0],[-2,0,3,180],[-1,0,0,90],[0,0,1,0],[1,0,2,90],[2,0,0,270],[-1,1,2,180],[0,1,1,270],[1,1,0,180],[0,2,3,0]];
 const placement=state.tilePlaced?'<div class="map-tile placed" style="--x:2;--y:1">'+tileSvg(1,state.tileRotation||0)+'</div>':'<button class="tile-placeholder" data-action="place-tile" style="--x:2;--y:1" aria-label="Đặt mảnh ghép vào ô này">'+icon('plus')+'</button>';
 return `<div class="tiles-map"><div class="map-grid" style="transform:scale(${state.tileZoom||1})">${cells.map(([x,y,t,r])=>`<div class="map-tile" style="--x:${x};--y:${y}">${tileSvg(t,r)}</div>`).join('')}${placement}<span class="meeple violet" style="--x:0;--y:0">♟</span><span class="meeple coral" style="--x:-1;--y:1">♟</span><span class="meeple green" style="--x:1;--y:-1">♟</span></div><div class="map-compass">N<span>↑</span></div><div class="map-help">${icon('plus')} Chọn ô nét đứt để đặt mảnh ghép</div></div>`;
}
export function gameArt(id,extra=''){
 if(id==='xiangqi')return `<div class="game-art art-xiangqi ${extra}" aria-hidden="true"><div class="art-grid"></div><span class="art-ring"></span><span class="art-token token-main">帥</span><span class="art-token token-other">馬</span><span class="art-token token-small">兵</span><span class="art-spark">✧</span></div>`;
 if(id==='chess')return `<div class="game-art art-chess ${extra}" aria-hidden="true"><div class="mini-checkers"></div><span class="hero-chess king">${chessPiece('K')}</span><span class="hero-chess knight">${chessPiece('n')}</span><span class="art-spark">✧</span></div>`;
 if(id==='caro')return `<div class="game-art art-caro ${extra}" aria-hidden="true"><div class="caro-art-grid"></div><span class="art-x x1">×</span><span class="art-o o1"></span><span class="art-x x2">×</span><span class="art-o o2"></span><span class="art-x x3">×</span></div>`;
 if(id==='tiles')return `<div class="game-art art-tiles ${extra}" aria-hidden="true"><div class="art-tile t1">${tileSvg(0,90)}</div><div class="art-tile t2">${tileSvg(1)}</div><div class="art-tile t3">${tileSvg(2,180)}</div><span class="art-meeple">♟</span></div>`;
 return `<div class="game-art art-werewolf ${extra}" aria-hidden="true"><span class="moon-disc"></span><span class="forest f1"></span><span class="forest f2"></span><span class="forest f3"></span><span class="wolf-eyes">• •</span><span class="night-star s1">✦</span><span class="night-star s2">✧</span></div>`;
}
