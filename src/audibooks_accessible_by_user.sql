/* SELECT * FROM audiobooks INNER JOIN (SELECT audiobook_id FROM library_permissions WHERE user_id == {}); */

(audiobooks JOIN libraries USING (audiobook.library_id, library.id)) JOIN library_permissions)
