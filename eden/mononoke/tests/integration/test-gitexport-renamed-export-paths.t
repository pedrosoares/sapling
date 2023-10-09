# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This software may be used and distributed according to the terms of the
# GNU General Public License found in the LICENSE file in the root
# directory of this source tree.

Setting up a simple scenario for the gitexport tool
  $ . "${TEST_FIXTURES}/library.sh"


Setup configuration
  $ REPOTYPE="blob_files"
  $ setup_common_config "$REPOTYPE"
  $ ENABLE_API_WRITES=1 REPOID=1 setup_mononoke_repo_config "temp_repo"
  $ cd $TESTTMP


Set some env vars that will be used frequently

  $ OLD_EXPORT_DIR="old_export_dir"
  $ EXPORT_DIR="export_dir"
  $ INTERNAL_DIR="internal_dir" # Folder that should NOT be exported to the git repo

# Test case to cover scenarios where an exported directory was created by
# renaming another.
# In this case, we want to follow the history and export the changesets affecting
# the directory with the old name.

# NOTE: Creating a history where there's an irrelevant commit (commit D)
# between one that modifies files in the old export path name (commit C) and
# the one that renames the export directory (commit E).
  $ testtool_drawdag -R repo --derive-all --no-default-files <<EOF
  > A-B-C-D-E-F-G-H-I-J-K
  > # modify: A "$OLD_EXPORT_DIR/B.txt" "File to export"
  > # message: A "Add files to export dir before rename"
  > # modify: B "$OLD_EXPORT_DIR/C.txt" "Another export file"
  > # message: B "Add another export file"
  > # modify: C "$OLD_EXPORT_DIR/C.txt" "Modify file to export"
  > # modify: C "$INTERNAL_DIR/another_internal.txt" "Internal file"
  > # message: C "Modify files in both directories"
  > # modify: D "$INTERNAL_DIR/internal.txt" "Internal file"
  > # message: D "Add file to internal_dir"
  > # copy: E "$EXPORT_DIR/B.txt" "File to export" D "$OLD_EXPORT_DIR/B.txt"
  > # copy: E "$EXPORT_DIR/C.txt" "Modify file to export" D "$OLD_EXPORT_DIR/C.txt"
  > # delete: E "$OLD_EXPORT_DIR/B.txt"
  > # delete: E "$OLD_EXPORT_DIR/C.txt"
  > # message: E "Rename export directory"
  > # modify: F "$INTERNAL_DIR/internal.txt" "Changing file"
  > # modify: F "$EXPORT_DIR/A.txt" "Changing file"
  > # message: F "Modify internal and exported files"
  > # modify: G "$EXPORT_DIR/B.txt" "Changing file"
  > # message: G "Modify only exported file"
  > # modify: H "$EXPORT_DIR/second_subdir_export.txt" "Changing file"
  > # message: H "Modify only file in export directory"
  > # modify: I "$INTERNAL_DIR/another_internal.txt" "Changing file"
  > # message: I "Modify only file in internal root"
  > # delete: J "$EXPORT_DIR/second_subdir_export.txt"
  > # delete: J "$INTERNAL_DIR/another_internal.txt"
  > # message: J "Delete internal and exported files"
  > # modify: K "root_file.txt" "Root file"
  > # message: K "Add file to repo root"
  > # bookmark: K master
  > EOF
  A=e954e5fb1ffc69119df10c1ed3218c1f28a32a1951d77367c868a57eb0ae8f53
  B=396a68afccbbf0d39c9be52eff16b3e87026de18468d15ee0e7dca9b33b97c2c
  C=4f918989900c17e32ee024fdcd634bb9beab540d7916c1941f737022baf41452
  D=659ed19d0148b13710d4d466e39a5d86d52e6dabfe3becd8dbfb7e02fe327abc
  E=6fc3f51f797aecf2a419fb70362d7da614bf5a7c1fc7ca067af0bdccff817493
  F=824be851b343d7d43e08d55b59a4bb57dadf7db4639044f79804764af286999a
  G=7f0bc8f6714d877194f074b9f8436bd3798cc183170d8707fb465e815807467b
  H=6b215d19cbf41a739e60176eac37c84bc50c118f5f4eb99bff5102f30a2ee617
  I=31de873264a0d07db554437559f01bd0827b84d051e8daa15c7f97d06693ff4a
  J=aeabdc90a1716382f1c7ebb4bb956339bb5cc12e0df11e8419266a37979839f2
  K=7616c9e240de5b549f4c1e5331d45419a783191c76a79bc6711c3eabd5148802

  $ start_and_wait_for_mononoke_server


# -------------------- Use the gitexport tool --------------------


  $ SOURCE_GRAPH_OUTPUT=$TESTTMP/source_graph_output
  $ PARTIAL_GRAPH_OUTPUT=$TESTTMP/partial_graph_output

Run the tool without passing the old name as an export path

  $ gitexport --log-level WARN --repo-name "repo" -B "master" -p "$EXPORT_DIR" --source-graph-output "$SOURCE_GRAPH_OUTPUT" --partial-graph-output "$PARTIAL_GRAPH_OUTPUT" --distance-limit 30
  *] Changeset ChangesetId(Blake2(6fc3f51f797aecf2a419fb70362d7da614bf5a7c1fc7ca067af0bdccff817493)) might have created one of the exported paths by moving/copying files from a previous commit that will not be exported (id ChangesetId(Blake2(659ed19d0148b13710d4d466e39a5d86d52e6dabfe3becd8dbfb7e02fe327abc))). (glob)

  $ diff --old-line-format="- %L" --new-line-format="+ %L" "$SOURCE_GRAPH_OUTPUT" "$PARTIAL_GRAPH_OUTPUT"
  - o  message: Add file to repo root, id: 7616c9e240de5b549f4c1e5331d45419a783191c76a79bc6711c3eabd5148802
  - │   File changes:
  - │  	 ADDED/MODIFIED: root_file.txt 1fc392f47d2822cab18c09dd980ea6bff4c0af4f55249fd01696b5ae04b8f30f
  - │
  - o  message: Delete internal and exported files, id: aeabdc90a1716382f1c7ebb4bb956339bb5cc12e0df11e8419266a37979839f2
  + o  message: Delete internal and exported files, id: 83e9cdda665675d07de4bf31599c5ebb6bd7d5a82d84376cb07c47644d3bdc47
  │   File changes:
  │  	 REMOVED: export_dir/second_subdir_export.txt
  - │  	 REMOVED: internal_dir/another_internal.txt
  - │
  - o  message: Modify only file in internal root, id: 31de873264a0d07db554437559f01bd0827b84d051e8daa15c7f97d06693ff4a
  - │   File changes:
  - │  	 ADDED/MODIFIED: internal_dir/another_internal.txt a6ef1a0dddad73cbfd4ce3bd9642f5aab0c4ae1fcb58af3cacda2f0ed914efd8
  │
  - o  message: Modify only file in export directory, id: 6b215d19cbf41a739e60176eac37c84bc50c118f5f4eb99bff5102f30a2ee617
  + o  message: Modify only file in export directory, id: c05252450958c57e340f66a5700aa098a080fd1f3e7852ddec0e9da29a7023e3
  │   File changes:
  │  	 ADDED/MODIFIED: export_dir/second_subdir_export.txt a6ef1a0dddad73cbfd4ce3bd9642f5aab0c4ae1fcb58af3cacda2f0ed914efd8
  │
  - o  message: Modify only exported file, id: 7f0bc8f6714d877194f074b9f8436bd3798cc183170d8707fb465e815807467b
  + o  message: Modify only exported file, id: 0806e1f098e9d7c770d9ea74d08317320375e0bd33f765397b0d7e614c3bdaac
  │   File changes:
  │  	 ADDED/MODIFIED: export_dir/B.txt a6ef1a0dddad73cbfd4ce3bd9642f5aab0c4ae1fcb58af3cacda2f0ed914efd8
  │
  - o  message: Modify internal and exported files, id: 824be851b343d7d43e08d55b59a4bb57dadf7db4639044f79804764af286999a
  + o  message: Modify internal and exported files, id: 63ea72001682f7b64323e7432f7e57497f74b8bfae4ea71e69621edf49530a38
  │   File changes:
  │  	 ADDED/MODIFIED: export_dir/A.txt a6ef1a0dddad73cbfd4ce3bd9642f5aab0c4ae1fcb58af3cacda2f0ed914efd8
  - │  	 ADDED/MODIFIED: internal_dir/internal.txt a6ef1a0dddad73cbfd4ce3bd9642f5aab0c4ae1fcb58af3cacda2f0ed914efd8
  - │
  - o  message: Rename export directory, id: 6fc3f51f797aecf2a419fb70362d7da614bf5a7c1fc7ca067af0bdccff817493
  - │   File changes:
  - │  	 COPY/MOVE: export_dir/B.txt 3e8ba6ef6107965afc1446b5b24533d9865204f1ea617672930d202f932bb892
  - │  	 COPY/MOVE: export_dir/C.txt 641106875cd2090a0019d25d920cf9015eb4036f1ece30b8fbb7dd5be785f9c4
  - │  	 REMOVED: old_export_dir/B.txt
  - │  	 REMOVED: old_export_dir/C.txt
  - │
  - o  message: Add file to internal_dir, id: 659ed19d0148b13710d4d466e39a5d86d52e6dabfe3becd8dbfb7e02fe327abc
  - │   File changes:
  - │  	 ADDED/MODIFIED: internal_dir/internal.txt dbc317c4f0146e8a455e9bc8eea646248145c962b3f4689c22285d3c8b25fd5e
  - │
  - o  message: Modify files in both directories, id: 4f918989900c17e32ee024fdcd634bb9beab540d7916c1941f737022baf41452
  - │   File changes:
  - │  	 ADDED/MODIFIED: internal_dir/another_internal.txt dbc317c4f0146e8a455e9bc8eea646248145c962b3f4689c22285d3c8b25fd5e
  - │  	 ADDED/MODIFIED: old_export_dir/C.txt 641106875cd2090a0019d25d920cf9015eb4036f1ece30b8fbb7dd5be785f9c4
  - │
  - o  message: Add another export file, id: 396a68afccbbf0d39c9be52eff16b3e87026de18468d15ee0e7dca9b33b97c2c
  - │   File changes:
  - │  	 ADDED/MODIFIED: old_export_dir/C.txt bc10fa4c7856280755c757a75dafadb36d7e5f105cdfeedbcdbc76dab37a708a
  │
  - o  message: Add files to export dir before rename, id: e954e5fb1ffc69119df10c1ed3218c1f28a32a1951d77367c868a57eb0ae8f53
  + o  message: Rename export directory, id: 916c09d10fe77c2b028ba5311c2d41257b042885d11a14f101598316ce25fda5
      File changes:
  -    	 ADDED/MODIFIED: old_export_dir/B.txt 3e8ba6ef6107965afc1446b5b24533d9865204f1ea617672930d202f932bb892
  +    	 ADDED/MODIFIED: export_dir/B.txt 3e8ba6ef6107965afc1446b5b24533d9865204f1ea617672930d202f932bb892
  +    	 ADDED/MODIFIED: export_dir/C.txt 641106875cd2090a0019d25d920cf9015eb4036f1ece30b8fbb7dd5be785f9c4
  [1]
