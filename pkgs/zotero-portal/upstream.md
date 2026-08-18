# zotero.org authenticated frontend notes

Observed against zotero.org on 2026-08-17 PDT using an isolated test account.
Page markup and dynamic behavior were checked against the JavaScript bundles
served by those pages. Group, invitation, role, library-setting, and API-key
forms were also submitted so their real responses could be observed.
Account-specific values, credentials, invitation and CSRF tokens, IP addresses,
and API keys are intentionally omitted.

Definitions used below:

- `gid`: numeric group ID
- `kid`: key identifier used by the settings UI
- `gname`: group name converted to Zotero's URL slug
- `username`: the user's profile/library slug

Backticks in the route sections denote literal visible text, including original
capitalization and punctuation. Prose outside backticks describes layout or
behavior and is not intended as replacement UI copy.

## Shared page shells

### Legacy site shell

The profile and group pages still use Zotero's older site theme.

- The body is `#e6e6e6` with a darker translucent band/gradient over its first
  200 px, is capped at 1060 px, and has 40 px bottom padding.
- The main content is a 900 px white panel with 16 px top/40 px horizontal
  padding, a 500 px minimum height, a `0 5px 10px rgba(0,0,0,.5)` shadow, and
  10 px rounded top corners.
- The legacy stylesheet uses Lucida Grande/Verdana at `87.5%` (roughly 14 px),
  1.4286 line height, and body text `#444`. H2 is `#666`; links are `#38c` and
  hover to `#369`.
- Buttons are bold white text on `#800`, with a one-pixel border, 4 px radius,
  4 px/8 px padding, a split highlight gradient, and a subtle dark text shadow.
  Hover uses `#900` and active uses `#a00`.
- Two-column pages use a floated 66% main column and 31% sidebar.
- Small contextual links are inline, separated by a middle dot, and have a
  dotted underline. The selected contextual link gets a solid underline.
- The top utility row contains the signed-in profile link followed by
  `Settings`, `Inbox`, `Download`, and `Log Out`, with a separate
  `Upgrade Storage` button.
- The primary navigation contains `Home`, `Web Library`, `Groups`,
  `Documentation`, `Forums`, and `Get Involved`, plus a site-search field and
  `Search` button.
- Breadcrumbs use `>` separators and appear above the page content.
- At 1040 px and below the background gradient and rounded corners disappear,
  the header gains 10 px side margins, and legacy pages impose an 840 px
  minimum width. The separate library-panel media rules can shrink to 300 px,
  but ordinary legacy profile/group pages remain substantially nonresponsive.

### Web Library shell

The personal and group libraries use the full Web Library application rather
than the legacy header/content/footer shell.

- The application fills the viewport and supplies its own desktop and mobile
  navigation.
- The production desktop menu contains `Web Library`, `Groups`,
  `Documentation`, `Forums`, and `Get Involved`.
- The user dropdown contains `My Profile`, `Inbox`, `Settings`, and `Logout`.
- `Upgrade Storage` is a right-aligned button on larger screens and a normal
  menu entry in the mobile navigation.
- The mobile menu flattens the user dropdown into individual entries.

### Modern settings shell

Settings pages use the newer responsive theme.

- Avenir Next is preferred, falling back to the system sans-serif stack. Body
  text is 16 px with a 1.5 line height and color `#222`; links are blue
  (`#0069e0`).
- The desktop header has the Zotero logo, `Web Library`, `Groups`,
  `Documentation`, `Forums`, and `Get Involved`; a user dropdown; and an
  `Upgrade Storage` button.
- The user dropdown contains `My Profile`, `Inbox`, `Settings`, and `Log Out`.
- Below the large-screen breakpoint, the header becomes a hamburger menu and
  the same destinations are shown in a dark slide-out navigation panel.
- Settings content sits on `#f6f6f6`. Individual settings sections are white
  cards with a 1 px light-grey border, 5 px radius, 24 px padding, and 24 px
  bottom spacing.
- The content container has 16 px side padding. Its exact maximum widths are
  536 px at the 568 px breakpoint, 736 px at 768 px, 972 px at 994 px, and
  1202 px at 1234 px.
- Every settings page has a `Home > Settings > Current Page` breadcrumb and an
  `Settings` H1.
- On large screens the settings navigation is a two-column-wide left sidebar.
  On smaller screens it becomes a horizontal row above the content. The active
  item has a light-grey background and bold text; the desktop version also has
  a blue left border.
- The observed settings tabs are `Profile`, `Email`, `Security`, `Privacy`,
  `Storage`, and `Read Aloud`.
- The footer is dark grey and contains links for documentation, forums, blog,
  privacy, terms, involvement, developers, and jobs.

## Library and profile routes

### `/mylibrary`

- Redirects to `/<username>/items`.
- The Web Library navigation marks `Web Library` active.

### `/<username>/items` and `/<username>/library`

- Both currently return the full personal Web Library.
- `/<username>/items` is the canonical target used by `/mylibrary` and by the
  legacy profile's `Library` link.
- `/<username>/library` is accepted as an alias.
- A direct personal item URL has the form
  `/<username>/items/itemKey/<item-key>`.

### `/groups/<gid>/<gname>/items`

- Returns the full Web Library focused on that group.
- A direct group item URL has the form
  `/groups/<gid>/<gname>/items/itemKey/<item-key>`.
- This `itemKey` path segment is also what the official Recently Added table
  uses for item-title links.

### `/<username>`

- Uses the legacy two-column shell.
- Main column:
  - the display name as H1
  - contextual links `Library` and, for the current user, `Edit Profile`
  - the `About you` value as unlabelled prose when present
  - labelled profile rows `Location`, `Disciplines`, and `Affiliation` when
    present
  - a Publications section populated asynchronously when the user has
    published items
- Sidebar:
  - profile image when present
  - an invitation-to-groups control when viewing another user
  - a `Groups` heading and compact group-name/member-count links
- The breadcrumb is `Home > People > <display name>` (falling back to the
  username). With an otherwise empty profile, the visible body reduces to the
  H1, `Library`, `Edit Profile`, and `Groups`.

## Group routes

### `/groups`

- Legacy shell and `Home > Groups` breadcrumb.
- H1: `Zotero Groups`.
- Contextual links: `Search for Groups` and `Create a New Group`.
- Main column contains the user's group list. The sidebar is reserved for group
  invitations and recent group discussions; empty widgets render nothing.
- Each group card contains:
  - optional group image
  - group name linked to `/groups/<gid>/<gname>`
  - `Group Library` linked to `/groups/<gid>/<gname>/items`
  - for owners/admins, `Manage Profile`, `Manage Members`, and `Manage Library`
  - a details table with `Members`, optional `Description`, `Group Type`, and
    `Group Library`
- Group-library access is described in prose derived from reading/editing
- Group type values are `Private`, `Public, Open Membership`, and
  `Public, Closed Membership`. Group-library values are selected from:
  - `Anyone can view, only members can edit`
  - `Anyone can view, only admins can edit`
  - `Only members can view and edit`
  - `Only members can view, only admins can edit`
  - `Only admins can view, only members can edit`
  - `Only admins can view and edit`
- With no groups, the list is replaced by a group explainer headed
  `What can groups do for you?` with the exact copy:
  - `With groups, you can collaborate remotely with project members, set up web-based bibliographies for classes you teach, and so much more.`
  - `Share your own work or sources you have discovered with others who are working in related areas.`
  - `Collaborate with colleagues, publicly or privately, on ongoing research.`
  - `Discover other people with similar interests and the sources they are citing.`
- Pending invitations render in the sidebar under `Group Invitations`. Each
  row shows the linked group title and `Join`/`Ignore` buttons.

Pagination is incremental rather than page-numbered:

- Initial request: `/users/<uid>/groups`, ordered by title, with `limit=25` and
  `start=0`.
- The response's `Total-Results` header is compared with the number accumulated
  in the browser.
- If more results exist, one `More` link is rendered below the cards.
- Activating it requests another 25 with `start=<number already loaded>`,
  appends those groups, and leaves the existing cards in place.
- The browser URL does not change, and there are no previous/next or numbered
  page controls.
- This behavior was verified both from the current production component and
  live with 26 memberships. The first request returned 25 rows and
  `Total-Results: 26`; the request with `start=25` returned the final row.

### `/search/type/group`

- Uses the modern responsive shell rather than the legacy group shell.
- H1: `Search Groups`.
- The search form has one text field and a `Search` button.
- It requests `/searchresults?type=group&query=<query>&page=<page>` and keeps
  `q` and `p` in the browser URL under `/search`.
- Results are displayed ten at a time as group cards. When more than ten exist,
  numbered page buttons are shown, with at most ten page numbers at once.
- The result endpoint returns `total` and `results`; each result contains the
  public group API object and website-only metadata.
- Each result is a bordered card. Its header contains the linked group name and
  optional image; the body starts with `Group Library`, then the same `Members`,
  optional `Description`, `Group Type`, and `Group Library` table used by the
  user's group cards. Manager links can add `Manage Profile`, `Manage Members`,
  and `Manage Library`.
- A completed search with zero results shows exactly `No results` in a
  borderless card with a top border. Private groups do not appear in this
  public search, even to one of their members.

### `/groups/new`

- Legacy full-width content panel.
- H1 and action links repeat the group-page create/search navigation.
- The page H1 is `Create a New Group`; contextual links are
  `Search for Groups` and `Create a New Group`.
- The first fieldset is `Group Name`, with the label
  `Choose a name for your group`.
- The second fieldset is `Group Type` and presents three large stacked choices:
  - `Public, Open Membership`
    - `Anyone can view your group online and join the group instantly.`
    - `Choose a Public, Open Membership`
  - `Public, Closed Membership`
    - `Anyone can view your group online, but members must apply or be invited.`
    - `Choose Public, Closed Membership`
  - `Private Membership`
    - `Only members can view your group online and must be invited to join.`
    - `Choose Private Membership`
- Public group names are checked asynchronously through
  `/groups/checkname?input=<name>` and the generated URL preview changes color
  based on validity.
- Private groups show a numeric group URL preview rather than a name-based
  public URL preview.
- The submit button is `Create Group`. Submitting no name rerenders the page
  with the top error `Invalid name`. A successful creation responds with `302`
  to `/groups/<new-gid>/settings/library`.
- The private-server portal intentionally only needs the private-membership
  case.

### `/groups/<gid>`

- Redirects to the group overview using the current group-name slug.

### `/groups/<gid>/<gname>`

- Legacy two-column group overview with a
  `Home > Groups > <group name>` breadcrumb.
- Main column:
  - group name as H1
  - `Group Library` and, for an authorized manager, `Group Settings`
  - `Recently Added Items`
- The Recently Added request loads the latest ten top-level items ordered by
  modification date descending.
- When items exist, they are shown in a striped table with `Title`, `Added By`,
  and `Date Modified` columns.
- A title links to
  `/groups/<gid>/<gname>/items/itemKey/<item-key>`.
- `Added By` prefers the creator's real/display name and falls back to the
  username.
- The footer says: `See all N items for this group in the Group Library.` The
  final words link to `/groups/<gid>/<gname>/items`.
- Empty text says that there are not yet any items in the collection. A user
  without viewing access instead sees that the library will be viewable after
  joining.
- Sidebar:
  - optional image, description, and external group URL
  - `Owner`, `Registered`, `Type`, `Membership`, and `Library Access`
  - membership values vary among invitation, closed, and open
  - access values distinguish view-and-edit, view-only, and no access
  - conditional join/leave/invitation/ownership controls
  - `Members (N)` linking to the public member list
- The sidebar metadata uses the literal labels `Owner:`, `Registered:`,
  `Type:`, `Membership:`, and `Library Access:`. Membership values are
  `Invitation`, `Closed`, or `Open`; Library Access values are
  `You can view and edit`, `You can only view`, or `None`.
- An invited user sees the exact copy
  `You have been invited to join this group.` followed by `Join` and `Ignore`.
  The production component reports a successful join as
  `You are now a member of this group`. It reports an ignored invitation or
  other processing failure as
  `There was a problem processing this action. Please try again in a few minutes.`
- The exact empty-library strings are
  `There are not yet any items in this collection` and
  `Library will be viewable after joining this group.`
- A live invited-user join submitted successfully as JSON
  `{ "success": true }`. The page refresh then exposed `Group Library`, changed
  `Members (1)` to `Members (2)`, and replaced the invitation controls with the
  member `Leave` control.

### `/groups/<gid>/<gname>/members`

- Legacy main column headed `<group name>: Members`.
- Members are rendered as a plain list of profile cards rather than a table.
- Each card has an optional square profile image, linked display name, and any
  available affiliation, disciplines, location, website, library, or C.V.
  links.
- The inspected page rendered the entire membership list and had no pagination
  controls.

### `/groups/<gid>/settings`

- Available to owners/admins; owner-only actions are kept separate.
- An ordinary member gets HTTP 403 on this route and on both
  `/settings/library` and `/settings/members`. All three use the modern error
  card with `Error`, `Access denied`, and
  `You do not have access to this area.`
- Legacy shell, H1 `<group name>: Settings`, and three contextual tabs:
  `Group Settings`, `Members Settings`, and `Library Settings`.
- Main-column `Group Information` form:
  - Group Name
  - Description, with help asking what the group is about
  - multi-select Disciplines grouped into academic categories
  - Group URL, with help asking whether the group has a webpage
  - Enable Comments
  - Save Settings
- Sidebar contains group-image editing.
- Owners additionally get an Owner Options section with deletion and transfer:
  - H2: `Owner Options`
  - `As the owner of this group, you can delete it.`
  - button: `Delete Group`
  - H3: `Transfer Ownership`
  - `Offer ownership of this group to another user. If accepted this will give them control of the group as well as utilize that user's storage for any group file storage.`
  - the eligible-member selector is `New Owner` and the submit control is
    `Transfer`
- Deleting a group uses the browser-native confirmation
  `This will permanently delete this group, including any items in the group library`.
  On acceptance, the legacy script rewrites the submitted button value to
  `confirmed`; there is no separate confirmation page. A successful deletion
  responds with `302 Location: /groups`. Posting the visible button value alone
  also redirects but does not delete the group. A `GET` to the form action
  currently returns the modern error page with `Error`,
  `Server Error`, and `Something went wrong processing your request.`
- Submitting an empty required group name keeps the form on the same page and
  displays `Value is required and can't be empty` beneath `Group Name`.
- A successful submission keeps the form on the same page and displays the top
  confirmation `Group Settings Saved`.

### `/groups/<gid>/settings/library`

- Legacy shell with the same three contextual settings tabs.
- Group Type:
  - `Private`
  - `Public Closed`
  - `Public Open`
  - `Controls who can see and join your group`
- Library Reading:
  - `Anyone on the internet`
  - `Any group member`
  - `Who can see items in this group's library?`
- Library Editing:
  - `Any group members`
  - `Only group admins`
  - `Who can add, edit, and remove items from this group's library?`
- File Editing:
  - `Any group members`
  - `Only group admins`
  - `No group file storage`
  - `Who can work with files stored in the group? Public Open groups cannot have file storage enabled.`
- The submit control is `Save Settings`.
- Public-open/file-storage and public-to-private transitions can require
  confirmation and attachment deletion before settings are saved. The browser
  confirmation strings are:
  - `Changing a group to 'Public Open' will disallow storing of files for the group. Before settings can be applied, all file attachments for the group will be deleted`
  - `Changing a group to 'Private' will no longer allow it to be read by the public`
- The attachment-removal flow uses `Choosing the selected settings requires deleting any stored attachment files for this library.`,
  `Checking all attachments`, `<N> attachments will be deleted.`,
  `Delete Attachments`, `No attachments need to be deleted`, and
  `Save Settings`.
- Save uses the website endpoint
  `/group/updatesettings?groupID=<gid>&settingsType=library`.
- A successful endpoint response is `{ "success": true }`; the production
  component then displays `Settings saved` and reloads the page. Its generic
  failure notification is `Error saving settings`, unless the response
  supplies a more specific `message`.

### `/groups/<gid>/settings/members`

- Legacy shell, H1 `<group name>: Members Settings`, and the same three tabs.
- `Current Members` is a table with `Username`, `Full Name`, `Member Since`, and
  `Role`.
- Usernames link to `/<username>`.
- The owner role is plain text: `Owner`. Every other member has a role selector
  with `Admin`, `Member`, and `Remove`.
- Role changes are applied together by the `Update Roles` button; changing a
  select does not immediately submit.
- The `Member Invitations` section lists pending invitations or an empty-state
  `No pending invitations.`, followed by `Send More Invitations` linking to
  `/groups/<gid>/invite`.
- A pending-invitation table uses the literal columns `Username`, `Full Name`,
  `Invited On`, and `Cancel Invitation`. Each row has a `Cancel` link.
- A successful role submission stays on this page, updates the selected values,
  and displays the top confirmation `Roles Updated`. There is no confirmation
  dialog.
- Public-closed groups can prepend `Member Applications`, with the columns
  `Username`, `Full Name`, `Applied On`, and `Action`; row actions are
  `Approve | Deny`.

### `/groups/<gid>/invite`

- Legacy full-width page headed `Invite Members` with a subheading
  `Group: <group name>`.
- The textarea label and submit button are both `Invite Members`.
- Help text: `Separate email addresses or zotero usernames with commas or lines.`
- Entries may be separated by commas or newlines. A successful invitation
  responds with `302` to `/groups/<gid>/settings/members`; the destination does
  not show a success banner and instead shows the new pending-invitation row.
- For an unknown username, the form is retained and the top error list says
  `Error sending invitation to "<submitted value>"` and
  `Other invitations have been sent`.

## Settings routes

### `/settings` and `/settings/profile`

- Both show the Profile settings content; `/settings` is not visibly a separate
  page.
- Content uses a seven-column main area plus a three-column image area beside
  the two-column settings navigation on large screens.
- `Username` initially shows:
  - `Login Username: <username>`
  - `Used to log in and through most of the site including groups and libraries.`
  - `Forums Display Name: <forum name>`
  - `Shown when posting to the Zotero forums.`
  - `Change Username`
- Editing exposes both fields, a live personal-profile URL preview, validation,
  and `Save`/`Cancel` controls. The editor labels are `Login Username` and
  `Forum Username`.
- Username action messages in the current component are `Username not changed`,
  `Your username has been updated`, `There was an error changing your username`,
  and `Username has been changed too recently`.
- `Public Profile` contains:
  - `Real Name` — `We're big on true identities at Zotero.`
  - `Location` — `We'll take a city, state, or country.`
  - `Disciplines` — `Hold the Control key (Windows/Linux) or Command key (Mac) to select more than one discipline.`
  - `Affiliation` — `What, if any, institution are you associated with?`
  - `About you` — `So what is it that you do around here?`
  - `Website` — `Do you have a website people should know about?`
  - `Save Profile`
- A successful public-profile submission stays on the page and displays
  `Profile Settings Saved` in the top success alert.
- A profile-image editor occupies the right column. Its literal controls are
  `Profile Image`, `Choose Image`, and, when an image exists, `Delete`.

### `/settings/email`

- Full-width settings content after the navigation.
- Under `Email Addresses`, the exact introductory copy is:
  - `You can log in using either your username or any of these email addresses.`
  - `All general account communications will go to your primary email address.`
  - `You can reset your Zotero password using any verified email address.`
- The dynamic manager uses `Primary`, `Verified`, `Unverified`,
  `Set As Primary`, `Verification email sent.`, `Resend`, and `Delete`.
  The add control is `Add email address`, placeholder `Additional Email`, and
  button `Add`.
- The one-verified-address prompt says:
  `You only have one verified email address associated with your account. If there's a risk you'll lose access to this account in the future, you may want to add a backup so you can always regain access to your Zotero account. Password reset emails can only be sent to verified addresses.`
- Under `Notifications`, the exact copy is:
  - `Notification emails will only be sent to your primary email address noted above.`
  - `Use the forum settings to update your notifications for Zotero forum posts or private messages.`
  - `Email me when:`
  - `someone invites me to a group`
  - `someone applies to a group I own`
  - `there is a new post in group discussion`
  - `Save Notification Settings`
- A successful notification submission displays `Notification Settings Saved`.

### `/settings/security`

- Full-width stack of white settings cards.
- `Password` has `Current Password`, `New Password`, and
  `Confirm New Password` fields and a `Change Password` button.
- `Two-Step Verification` says:
  `Two-step verification, also called two-factor authentication or multi-factor authentication, adds an additional layer of security to your account by requiring more than just a password to sign in.`
- Its dynamic sections are `Email Authentication`, `Authenticator App`, and
  `Passkeys & Security Keys`. Their explanatory text is:
  - `Receive an authentication code emailed to: <primary email>`
  - `Use an authenticator app or browser extension to get two-factor authentication codes when prompted.`
  - `Register a passkey to sign in without a password, or a hardware security key to use as your second factor.`
  Controls vary between `Enable`/`Disable` and `Edit`/`Hide`.
- `Browser Sessions` is followed by the exact text:
  `These are active sessions that are logged in to this account in a browser on zotero.org.`
- The browser-session table columns are `Platform`, `Last Used`,
  `Last Used IPs`, `Expires`, and `Delete Session`.
- The current row is marked `Current` in the Platform cell, uses `Now` for
  recent activity, and has a `Log Out` action. Other rows use relative times
  and `Remove Session`.
- The muted note says:
  `The locations shown are estimated from the IP address assigned to the device by the internet service provider. Estimations based on IP address can be inaccurate, so you might see a nearby city instead of your actual location.`
- `Applications` is followed by the exact text:
  `These are keys used by official Zotero applications or third party applications to access your synced data.`
- It contains `Create new private key` and
  `User ID: Your user ID for use in API calls is <uid>`.
- The striped key table columns are `Name`, `Last Used`, `Last Used IPs`,
  `Edit Key`, and `Delete Key`. Row actions use those same captions.
- Key editing links to `/settings/keys/edit/<kid>`; deletion is a POST form.
- Upstream has no disconnect-all/sign-out-all control.
- A final red-accented account-deletion section explains the consequences and
  uses the literal text:
  - `Delete Account`
  - `Permanently deleting your account will prevent it from being used in the future.`
  - `This should not be used for troubleshooting, or if you plan to continue using Zotero. If you are syncing a Zotero application, you will not be able to sync with a different account without deleting your data.`
  - `If you're simply trying to change your username, you can do so from your profile settings.`
  - `Permanently Delete Account`
  This is not relevant when identity is owned by an external OIDC provider.

### `/settings/privacy`

- One settings card contains:
  - `Publish entire library`
  - `This will make all items in your library viewable by anyone visiting zotero.org. Your notes will not be viewable unless you select the option below.`
  - `Publish notes`
  - `This will make all notes in your library viewable by anyone visiting zotero.org. Be very sure you want to do this.`
  - `Hide from search engines`
  - `We will tell search engines not to index your zotero.org profile page.`
  - `Update Settings`
- A successful submission displays `Privacy Settings Saved`.
- These controls do not make sense for a private-only portal.

### `/settings/storage`

- Full-width settings content.
- Starts with `Frequently Asked Questions`.
- A dynamic storage section has a two-column plan layout on larger screens:
  - `Current Plan` table containing Quota, Expiration, and Current Usage
  - usage lines for My Library, each owned group, and Total
  - usage-percentage progress bar for finite quotas
  - renewal/payment rows when applicable
  - `Change Plan` table with storage amount, annual price, and plan action
- Free plans show no expiration and no payment controls. Paid/institutional
  plans add renewal, payment, invoice, and institution details.
- The commercial UI is followed by `Subscriptions are billed annually.` and
  `By using Zotero, you agree to be bound by its Terms of Service.`
- The red danger button is `Purge Storage in My Library`; its help is
  `Use this to free up space for group libraries you own after disabling file syncing for My Library or switching to WebDAV storage.`
- The modal is headed `Purge File Storage` and says:
  - `This will permanently delete all files you have synced to your personal library (My Library) on the Zotero servers.`
  - `When to use this: You have disabled file syncing for My Library in your sync preferences or switched to WebDAV file syncing, and you want to free up Zotero storage space for group libraries you own.`
  - `This does not:`
  - `Delete any items, notes, or other data from your Zotero library`
  - `Remove local files from your computer or other devices`
  - `Affect files in group libraries`
  - `If you are still using Zotero file syncing for My Library, do not use this. Your files would be re-uploaded on the next sync.`
  - actions `Cancel` and `Purge Storage`

### `/settings/readaloud`

- H2: `Read Aloud`.
- `Standard Voices` lists:
  - `Natural-sounding voices`
  - `Only available with an internet connection`
  - `Requires a Zotero account`
  - `Limited language selection`
  - `No multilingual support`
  - `Source text doesn't leave Zotero servers`
  - `Unlimited use with a Zotero Storage subscription`
- `Premium Voices` lists:
  - `Highest-quality voices`
  - `Only available with an internet connection`
  - `Requires a Zotero account`
  - `Broad language selection`
  - `Multilingual support`
  - `Source text is processed by external text-to-speech providers`
  - `Zotero Storage plans include monthly Premium Voice minutes`
- The balances panel uses `Standard Voice Minutes Remaining:`,
  `Premium Voice Minutes Remaining:`, and
  `Premium voice minutes reset each month.`
- This is unrelated to the private account/library portal.

### `/settings/cv`

- Currently responds with `302 Location: /settings/profile`. There is no C.V.
  editor or `C.V.` settings tab in the pages observed in this investigation.

## API-key routes

### `/settings/keys/new`

- Uses the modern settings shell with the Security tab active and a centered
  eight-column editor.
- Heading: `New Key`.
- The key-name field is `Key Name`.
- A `Permissions` summary card uses these literal list-item forms as access is
  selected:
  - `Access to read your personal library`
  - `Access to read notes in your personal library`
  - `Access to modify your personal library`
  - `Access to read any of your group libraries`
  - `Access to read and modify any of your group libraries`
  - `Access to read library for group "<name>"`
  - `Access to read and modify library for group "<name>"`
  - `No additional permissions selected`
- Personal Library card:
  - `Personal Library`
  - `Allow library access` — `Allow third party to access your library.`
  - `Allow notes access` — `Allow third party to access your notes.`
  - `Allow write access` — `Allow third party to make changes to your library.`
- Default Group Permissions card:
  - `Default Group Permissions`
  - `All Groups`
  - `None`, `Read Only`, or `Read/Write`
  - `Allow access to all current and future groups.`
- Specific Groups card:
  - `Specific Groups`
  - `Per Group Permissions` checkbox
  - `Set group by group permissions for this key`
  - when enabled, each group gets None, Read Only, and Read/Write choices
  - each group is followed by `Access level to this group.`
- Save Key submits JSON to `/settings/savekey`.
- Revoke Key is only meaningful for an existing key; upstream still renders the
  control through the shared editor.
- Buttons are `Save Key` and `Revoke Key`.
- `Manage all access keys` links to `/settings/security#applications`.
- The client-side empty-name validation is `Key is missing name`; the generic
  save failure is `Error saving key`.
- After successful creation, the editor is replaced by the exact text
  `Your new API key has been created and is displayed below. Please save it now as it will not be accessible again after this point.`
  and a centered green panel containing the secret. The permissions and buttons
  disappear. Although the component sets a `Key Saved` notification internally,
  that notification is not in the successful-creation branch that is rendered.
- The server injects the complete user-group list into this page. It does not
  use the 25-at-a-time `/groups` display pagination.

### `/settings/keys/edit/<kid>`

- Same editor, headed `Edit Key`, prefilled from the selected key.
- Saving POSTs JSON to `/settings/savekey?key=<kid>` and shows a green
  `Key Saved` notification without replacing the editor.
- The current component intends to show `Key Revoked` and return to the
  Applications section. Its failure text is `Error deleting key`.
- The live `Revoke Key` contract appears broken upstream as of the observation
  date: the component-provided 24-character key value produced HTTP 404 with
  `{ "success": false, "error": "Key not found" }`, and the key remained
  listed. The separate Security-page `Delete Key` form worked and returned
  `302 Location: /settings/security#applications`.

The save payload is shaped as:

```json
{
  "key": "<kid-or-null>",
  "name": "<name>",
  "access": {
    "user": {
      "library": true,
      "notes": true,
      "files": true,
      "write": true
    },
    "groups": {
      "all": {
        "library": true,
        "write": false
      }
    }
  }
}
```

`files` exists in the wire representation even though the current official
private-key editor does not expose a separate files checkbox. Live successful
create and edit responses omitted permissions whose value was false. They also
returned `files: true` whenever personal-library access was enabled. In the
edit probe, sending an empty `groups.all` object was normalized by the server to
read access (`library: true`, `write: false`) instead of being returned empty.

## Non-page endpoint contracts

`Observed` below means the endpoint was sent a real request during this
investigation. `Bundle` means the contract was read from the exact JavaScript
bundle served by the page, but that mutation was not sent. Browser-generated
`multipart/form-data` boundaries are omitted.

### Group discovery and library reads

- `GET /searchresults?type=group&query=<string>&page=<positive integer>`
  (`Observed`)
  - no request body; cookie credentials are included
  - response: `application/json`
  - shape: `{ "total": <integer>, "results": [<group search result>...] }`
  - the group search UI uses ten results per page
- `GET /groups/checkname?input=<string>` (`Observed`)
  - no request body
  - response: `application/json`
  - shape:
    `{ "input": <string>, "slug": <string>, "valid": <boolean> }`
- `GET https://api.zotero.org/users/<uid>/groups` (`Observed`)
  - query: `session=<website session>`, `order=title`, `limit=25`,
    `start=<nonnegative integer>`
  - no request body; the component requests with `credentials: omit`
  - response: `application/json`, an array of Zotero group objects
  - `Total-Results` gives the full count; `Link` includes the alternate website
    route
- `GET https://api.zotero.org/groups/<gid>/items/top` (`Observed`)
  - query generated by Zotero's API URL builder: `limit=10` and
    `order=dateModified`; authenticated private-library requests include a
    website-session credential
  - no request body; the component requests with `credentials: omit`
  - response: `application/json`, an array of top-level Zotero item objects;
    `Total-Results` gives the full item count
- `GET /groups/invitations` (`Observed`)
  - no request body; authenticated by the website cookie
  - response: `application/json`
  - shape:
    `{ "invitations": [<invitation>...], "invitationGroups": { "<gid>": <group wrapper>, ... } }`
  - invitation objects contain a numeric group ID, numeric user ID, invitation
    state, date, optional email, and an opaque token
- `GET /storage/usersubscription` (`Observed`)
  - cookie-authenticated, no body, `application/json`
  - top-level fields: `userSubscription`, `storageGroups`, `planQuotas`,
    `lastPurchaseQuota`, and `intentClientSecret`
  - `userSubscription` contains numeric quota/level/usage fields, expiration,
    recurrence and payment state, institutions, and the numeric user ID
- `GET /storage/getusercustomer` (`Observed`)
  - cookie-authenticated, no body, `application/json`
  - shape: boolean `success` plus nullable `stripeCustomer`
- `GET /storage/lastsync` (`Observed`)
  - cookie-authenticated, no body, `application/json`
  - shape: `{ "lastSyncData": null|<last-sync object> }`

### Group mutations

- `POST /groups/new` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `name=<string>`,
    `group_type=PublicOpen|PublicClosed|Private`, `submit=Create Group`
  - failure: HTML `200`; success: `302` to the new group's Library Settings
- `POST /groups/<gid>/settings` (`Observed`)
  - `Content-Type: multipart/form-data`
  - body fields: `name`, `description`, repeated `disciplines[]`, `url`,
    `enable_comments=0|1`, `lastModified`, and
    `settings_submit=Save Settings`
  - response: HTML `200`, with inline validation or a top success message
- `POST /group/updatesettings?groupID=<gid>&settingsType=library` (`Observed`)
  - `Content-Type: multipart/form-data`
  - body:
    `type=Private|PublicClosed|PublicOpen`,
    `libraryReading=all|members`,
    `libraryEditing=members|admins`, and
    `fileEditing=members|admins|none`
  - authenticated by cookie credentials plus the raw website session in the
    `Authorization` header
  - response: `application/json`; success is `{ "success": true }`; failures
    may add `message`
- `POST /groups/<gid>/settings/members` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body contains one field per editable numeric user ID, named by its decimal
    ID and valued `admin|member|remove`, plus
    `submit_roles=Update Roles`
  - response: HTML `200`; success contains `Roles Updated`
- `POST /groups/<gid>/invite` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `invite_members=<comma-or-newline-separated string>`,
    `submit=Invite Members`, `verify_session=<CSRF token>`
  - success: `302` to `/groups/<gid>/settings/members`; validation failure:
    HTML `200`
- `POST /groups/inviteuser` (`Bundle`)
  - `Content-Type: multipart/form-data`
  - body: numeric `groupID`, numeric `userID`, and `ajax=true`
  - response: `application/json` with boolean `success`
- `GET /groups/<gid>/revokeinvite/<opaque token>` (`page markup`)
  - no body; this is the `Cancel` link in the pending-invitations table
- `GET /groups/<gid>/approve/<uid>` and
  `GET /groups/<gid>/deny/<uid>` (`Bundle`)
  - no body; `uid` is numeric
  - these are legacy mutation links for public-closed membership applications
- `POST /groups/<gid>/join` (`Observed`)
  - `Content-Type: multipart/form-data`
  - body: `ajax=true`; an invited user also sends the opaque `token`
  - authenticated by the website cookie
  - response: `application/json`; a successful invitation is
    `{ "success": true }`, while a public-closed application can instead set
    `pending: true`
- `POST /groups/<gid>/decline` (`Bundle`)
  - `Content-Type: multipart/form-data`
  - body: `ajax=true`, `token=<opaque invitation token>`
  - response: `application/json` with boolean `success`; the UI then navigates
    to `/groups`
- `POST /groups/<gid>/leave` (`Bundle`)
  - `Content-Type: multipart/form-data`
  - body: `ajax=true`
  - cookie credentials plus raw-session `Authorization` header
  - response: `application/json` with boolean `success`
- `POST /groups/<gid>/delete` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body after the browser confirmation: `confirm_delete=confirmed`
  - success: `302 Location: /groups`
  - `confirm_delete=Delete Group` also redirects but leaves the group intact
- `POST /groups/<gid>/settings` for ownership transfer (`page markup`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: numeric `newOwner` and `owner_submit=Transfer`
- `POST /groups/<gid>/settings/image` (`Bundle`)
  - `Content-Type: multipart/form-data`; body field `profile_image=<file>`
  - cookie credentials plus raw-session `Authorization` header
  - files over 512 KiB are rejected in the browser before the request
  - response: `application/json` with boolean `success`
- `DELETE /groups/<gid>/settings/image` (`Bundle`)
  - no body; cookie credentials plus raw-session `Authorization` header
  - response: `application/json` with boolean `success`

### Key mutations

- `POST /settings/savekey` for creation and
  `POST /settings/savekey?key=<24-character key>` for editing (`Observed`)
  - the bundle passes `JSON.stringify(...)` directly to `fetch`, which results
    in a raw JSON string body (normally `text/plain;charset=UTF-8` in a browser);
    the live endpoint also accepted `application/json`
  - cookie credentials plus raw-session `Authorization` header
  - body:

    ```json
    {
      "key": false,
      "name": "Key name",
      "access": {
        "user": {
          "library": true,
          "notes": true,
          "files": true,
          "write": true
        },
        "groups": {
          "all": { "library": true, "write": false },
          "12345": { "library": true, "write": true }
        }
      }
    }
    ```

  - on edit, `key` is the same 24-character value as the query parameter
  - JSON object keys are necessarily strings; application code should parse a
    specific-group key such as `"12345"` into a numeric group ID rather than
    treating the domain ID itself as a string
  - response: `application/json`; ordinary success has boolean `success` and an
    `updatedKey` object, with optional `verifier` and `redirect`
- `POST /settings/revokekey?key=<24-character key>` (`Observed`)
  - `Content-Type: multipart/form-data`
  - body: `revoke=1`, `key=<same key>`
  - cookie credentials plus raw-session `Authorization` header
  - intended response: JSON boolean `success`; the live result during this
    investigation was the broken 404 described above
- `POST /settings/keys/delete/<12-character kid>` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `verify_session=<CSRF token>`
  - success: `302 Location: /settings/security#applications`

### Account settings mutations

- `POST /settings/profile` (`Observed`)
  - `Content-Type: multipart/form-data`
  - body fields: `profile_realname`, `profile_location`, repeated
    `profile_disciplines[]`, `profile_affiliation`, `profile_bio`, optional
    `profile_website`, `submit=Save Profile`, and `verify_session`
  - response: HTML `200`
- `POST /settings/profileimage` (`Bundle`)
  - `Content-Type: multipart/form-data`; body field `profile_image=<file>`
  - cookie credentials plus raw-session `Authorization` header
  - files over 512 KiB are rejected in the browser before the request
  - response: `application/json` with boolean `success`
- `DELETE /settings/profileimage` (`Bundle`)
  - no body; cookie credentials plus raw-session `Authorization` header
  - response: `application/json` with boolean `success`
- `POST /settings/changeusername` (`Bundle`)
  - `Content-Type: multipart/form-data`
  - body: `username=<string>`, `forumUsername=<string>`
  - cookie credentials plus raw-session `Authorization` header
  - response: `application/json` with boolean `success`; HTTP 429 is handled as
    the too-recent case
- `GET /user/checkusername?username=<percent-encoded string>` (`Bundle`)
  - no body; JSON validation response
- `POST /settings/updateemail` (`Bundle`)
  - `Content-Type: multipart/form-data`
  - body always includes `ajax=true` and
    `emailAction=add|delete|primary|validate`; actions that target an address
    also include `email=<string>`
  - cookie credentials plus raw-session `Authorization` header
  - response: `application/json` with boolean `success`
- `POST /settings/email` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `email_invitation=0|1`, `email_application=0|1`,
    `email_groupmessage=0|1`, and `submit=Save Notification Settings`
  - response: HTML `200`
- `POST /settings/privacy` (`Observed`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `privacy_publishLibrary=0|1`, `privacy_publishNotes=0|1`,
    `privacy_hideFromSearchEngines=0|1`, `verify_session`, and
    `submit_privacy=Update Settings`
  - response: HTML `200`
- `POST /settings/sessions/delete/<session-id>` (`page markup`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `verify_session=<CSRF token>`
- `POST /settings/storage` for the purge form (`page markup`)
  - `Content-Type: application/x-www-form-urlencoded`
  - body: `purge_storage=1`, `verify_session=<CSRF token>`
  - the page sets `purge_storage` only after the user activates the modal's
    final `Purge Storage` button

## Authentication detail

`zotero_www_session_v2` is a website session credential, not a normal Zotero
Web API key. Authenticated legacy pages also receive a distinct API key in their
page configuration. Some legacy web components authenticate API requests by
passing the website session value as a `session` query parameter. Neither value
should be logged, committed, or exposed to client code outside the authenticated
page that requires it.
