/* design/sidebar-account.jsx — the profile line at the bottom of the left
   sidebar (photo, name); a click opens Settings → Account. */

function SidebarAccount({ avatar, name, onClick }) {
  const Avatar = window.AccountAvatar;
  return (
    <button type="button" className="psb-account" onClick={onClick} title="Account settings">
      <Avatar src={avatar} name={name} size={32} />
      <span className="psb-account-name">{name || window.OMP_I18N.t("Your profile")}</span>
      <svg className="psb-account-chev" width="12" height="12" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

Object.assign(window, { SidebarAccount });
