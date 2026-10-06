/* Public identity fixture/resolver shared by dashboard reference and game board.
 * No role assignment, game art or account/network contract is inferred here. */
export const fixture=await (await fetch('./avatar-fixtures.json')).json();
export const profiles=fixture.profiles;
export function publicAvatarUrl(profile){
 const ref=profile.avatar?.url||profile.avatar?.assetRef;
 if(!ref)return null;
 try{const url=new URL(ref,document.baseURI);if(url.origin===location.origin&&['http:','https:'].includes(url.protocol))return url.href;}catch{}
 return null;
}
const mountVersions=new WeakMap();
export function mountAvatar(container,profile){
 const epoch=(mountVersions.get(container)||0)+1;mountVersions.set(container,epoch);
 container.querySelectorAll('img[data-account-avatar],.avatar-initials').forEach(node=>node.remove());container.classList.remove('avatar-fallback');
 container.dataset.subjectId=profile.subjectId;container.dataset.avatarRef=profile.avatar?.assetRef||'';
 const fallback=document.createElement('span');fallback.className='avatar-initials';fallback.textContent=profile.initials||profile.displayName?.slice(0,1)||'?';fallback.setAttribute('aria-hidden','true');container.append(fallback);
 const url=publicAvatarUrl(profile);if(!url)return;
 const img=document.createElement('img');img.dataset.accountAvatar='true';img.referrerPolicy='same-origin';img.alt=`Avatar tài khoản mẫu ${profile.displayName}`;img.src=url;img.decoding='async';img.addEventListener('error',()=>{if(mountVersions.get(container)!==epoch||!container.contains(img))return;img.remove();container.classList.add('avatar-fallback');},{once:true});img.addEventListener('load',()=>{if(mountVersions.get(container)===epoch&&container.contains(img))fallback.hidden=true;},{once:true});container.append(img);
}
