/* Compatibility adapter. Public account-avatar fixtures; no masks or role data. */
import {profiles,publicAvatarUrl} from './account-avatars.mjs';
export const names=profiles.map(p=>p.displayName);
export const portraits=profiles.map(publicAvatarUrl);
export {profiles};
