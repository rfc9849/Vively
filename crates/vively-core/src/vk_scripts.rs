pub const PLAYLIST_CONTENTS: &str = r#"var owner_id = Args.owner_id;
var playlist_id = Args.playlist_id;
var access_key = Args.access_key;
var count = Args.count;
var offset = Args.offset;
var res = {};
if (offset == 0) {
  var info = API.audio.getPlaylistById({
    owner_id:owner_id,
    playlist_id:playlist_id,
    access_key:access_key,
    extended:1,
    extra_fields:"owner",
  });
  res.playlist = info.playlist;
  var creator = owner_id;
  if (res.playlist.original) {
    creator = res.playlist.original.owner_id;
  }
  if (creator < 0) {
    var group = API.groups.getById({ group_id: -creator }).groups[0];
    res.playlist.owner = group;
    res.owner = { id: creator, first_name: group.name, last_name: "", photo_base: group.photo_200, is_closed: false };
  } else {
    var user = null;
    if (!res.playlist.original && info.profiles) {
      user = info.profiles[0];
    } else {
      user = API.users.get({ user_ids: creator, fields: "photo_base" })[0];
    }
    res.playlist.owner = user;
    res.owner = user;
  }
  res.owner_audio_open = API.audio.get({ owner_id: creator, count: 1 }) ? true : false;
}
res.audios = API.audio.get({
  owner_id: owner_id,
  playlist_id: playlist_id,
  access_key: access_key,
  count: count,
  offset: offset,
});
res.audios.offset = offset;
return res;"#;

pub const AUDIO_HOME: &str = r#"var catalogs = API.catalog.getAudio({ need_blocks: 0 }).catalog.sections;
var res = API.catalog.getSection({ need_blocks: 1, section_id: catalogs[0].id });
var nextFrom = res.section.next_from;
if (nextFrom) {
  var more = API.catalog.getSection({ need_blocks: 1, section_id: catalogs[0].id, start_from: nextFrom });
  res.section.blocks = res.section.blocks + more.section.blocks;
  if (more.playlists) { res.playlists = (res.playlists ? res.playlists : []) + more.playlists; }
  if (more.audios) { res.audios = (res.audios ? res.audios : []) + more.audios; }
}
res.mixSettings = API.audio.getStreamMixSettings({ mix_id: "common" });
var rp = res.recommended_playlists;
if (rp) {
  var out = [];
  var i = 0;
  while (i < rp.length && i < 20) {
    var info = API.audio.getPlaylistById({ owner_id: rp[i].owner_id, playlist_id: rp[i].id, access_key: rp[i].access_key });
    out.push(rp[i] + { title: info.title, photo: info.photo, thumbs: info.thumbs, access_key: info.access_key });
    i = i + 1;
  }
  res.recommended_playlists = out;
}
return res;"#;

pub const SEARCH_IN_PROFILE: &str = r#"return { playlists: API.audio.searchPlaylists({ owner_id: Args.owner_id, q: Args.query, count: Args.p_count, filters: "owned", extended: 1 }), audios: API.audio.search({ search_own: 1, owner_id: Args.owner_id, q: Args.query, count: Args.a_count }).items };"#;
