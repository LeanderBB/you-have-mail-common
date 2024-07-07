mod utils;
use crate::utils::login;
use proton_api::domain::LabelType;
use proton_api::requests::GetLabelsRequest;
use utils::{create_session_and_server, DEFAULT_USER_EMAIL, DEFAULT_USER_PASSWORD};

#[test]
fn session_label_fetch() {
    let (client, server) = create_session_and_server();

    let (user_id, _) = server
        .create_user(DEFAULT_USER_EMAIL, DEFAULT_USER_PASSWORD)
        .expect("failed to create default user");

    let folder_id = server
        .create_label(&user_id, "my_folder", None, LabelType::Folder as i32)
        .expect("Failed to create folder");

    let label_id = server
        .create_label(&user_id, "my_label", None, LabelType::Label as i32)
        .expect("Failed to create folder");

    let session = login(client, DEFAULT_USER_EMAIL, DEFAULT_USER_PASSWORD);
    {
        let folders = session
            .execute_with_auth(GetLabelsRequest::new(LabelType::Folder))
            .expect("Failed to get folder labels")
            .labels;
        assert_eq!(1, folders.len());
        assert_eq!(folder_id.as_ref(), folders[0].id.0);
        assert_eq!("my_folder", folders[0].name);
    }

    {
        let labels = session
            .execute_with_auth(GetLabelsRequest::new(LabelType::Label))
            .unwrap()
            .labels;
        assert_eq!(1, labels.len());
        assert_eq!(label_id.as_ref(), labels[0].id.0);
        assert_eq!("my_label", labels[0].name);
    }
}
