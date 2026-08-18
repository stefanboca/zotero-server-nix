gid = group id
kid = key id
gname = group name

- /groups
  - search
  - create
  - list of groups
    - link to /groups/<gid>/<gname>/items
    - links to /groups/<id>/settings{,/library,/members}
    - number of members
    - type
    - group library (ex only members can view and edit)
- /groups/new
  - name
  - type
    - Public, Open Membership
      Anyone can view your group online and join the group instantly.
      (default)
    - Public, Closed Membership
      Anyone can view your group online, but members must apply or be invited.
    - Private Membership
      Only members can view your group online and must be invited to join.
  - invalid name if name conflicts with site routes (ex settings)
  - "Create Group" opens /groups/<gid>/settings/library
- /groups/<id>/settings
  - name
  - description
  - disciplines
  - group url
  - enable comments
  - delete group
  - transfer ownership
- /groups/<id>/settings/members
  - table of current members
    - username (link to /<username>), full name, member since, role (everyone except owner has a dropdown with admin/member/remove)
  - member invitations
    - pending invitations
    - send more invitations button
- /groups/<gid>/settings/library
  - type (controls who can see and join your group)
  - library reading (who can see items in this group's library)
    - anyone on the internet
    - any group member
  - library editing (who can add, edit, remove items)
    - any group member
    - only group admins
  - file editing (who can work with files stored in the group? public open groups cannot have file storage enabled)
    - any group member
    - only group admins
    - no group file storage
- /groups/<gid>/<gname>
  - link to /groups/<gid>/<gname>/items
  - link to /groups/<gid>/settings
  - list of recently added items
  - owner
  - registered date
  - type
  - membership (ex invitation)
  - library access
  - link to /groups/<gid>/<gname>/members
- /groups/<gid>/<gname>/members
  - list of group members, each user links to /<username>
- /groups/<gid>/<gname>/items
  - web library page
- /settings
  - same content as /settings/profile
- /settings/profile
  - username (login username, forums display username)
  - public profile
    - real name
    - location
    - disciplines
    - affiliation
    - about you
- /settings/security
  - browser sessions table
    - platform, last used (relative, ex now, 4 days ago, 4 years ago), last used ips (format: ip addr (location)), expires (absolute, ex 2026-08-18 01:54:28), delete session (for current, is "Log Out", for other, is "Remove Session")
  - applications
    - create new private key button, links to /settings/keys/new
    - shows numeric user id (ex "User ID: Your user ID for use in API calls is 0123456")
    - table
      - name, last used (relative), last used ips, edit key (link to /settings/keys/edit/<kid>), delete key
- /settings/storage
  - current plan
    - quota
    - expiration (can be Never, ex for free plan)
    - current usage (MB)
      - My Library
      - each group
      - Total
    - purge storage in My Library
      - "Use this to free up space for group libraries you own afer disabling file syncing for My Libraries or switching to WebDAV storage"
- /settings/keys/new
  - name
  - permissions
    - personal library (checkboxes)
      - library access
      - notes access
      - write access
    - group permissions (all groups)
      - radio: none, read only, read/write
      - "Allow access to all current and future groups."
    - specific groups
      - checkbox "Per Group Permissions"
      - checking reveals, for each group, radio none,read only, read/write
    - save key and revoke key buttons (revoke key seems to do nothing, even on /settings/keys/<kid> page.)
    - after submitting, make a post request, location remains the same but content changes to
      - "Your new API key has been created and is displayed below. Please save it now as it will not be accessible again after this point." and key in a green box
    - manage all access keys link -> /settings/security#applications
- /settings/keys/edit/<kid>
  - same layout as /settings/keys/new, except save key adds a green box that says "Key Saved"

all settings pages have a nav sidebar on the left with a header "Settings" and below Profile, Email, Security, Privacy, Storage, Read Aloud

all pages have breadcrumbs below nav bar, ex "Home > Settings > Security > Edit Key" where everything except top level is a link.
