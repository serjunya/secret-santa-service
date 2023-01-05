// # Веб-сервис секретного Санты.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tide::{Request, Response};
use serde_json::{Value, json, Map};

#[derive(PartialEq,Eq)]
enum Access
{
    User,
    Admin,
}

type Id = u32;

#[derive(Eq, Hash, PartialEq)]
struct UserGroupId
{
    user_id: Id,
    group_id: Id,
}
struct UserGroupProps
{
    access_level: Access,
    santa_id: Id,
}

struct DataBase
{
    users: HashMap<Id, String>,
    groups: HashMap<Id, bool>,
    user_groups: HashMap<UserGroupId, UserGroupProps>,
}

fn get_field<T>(object: &serde_json::Map<String, Value>, key: &str) -> T
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    object.get(key).unwrap().as_str().unwrap().parse().unwrap()
}

fn get_not_used_in_map_id<T>(map: &HashMap<Id, T>) -> Id
{
    match map.keys().max()
    {
        Some(id) => id + 1,
        None => 0,
    }
}

fn response_data(value: Value) -> Response
{
    Response::builder(200)
        .body(tide::Body::from_json(&value).unwrap())
        .build()
}

fn response_empty() -> Response
{
    Response::builder(200).build()
}

fn response_error(msg: &str) -> Response
{
    Response::builder(400)
        .body(tide::Body::from_json(&json!({"error": msg})).unwrap())
        .build()
}




fn user_create(input_obj: &Map<String, Value>, state: &Arc<Mutex<DataBase>>) -> Response
{
    let name: String = get_field(input_obj, "name");
    if name.len() > 0
    {
        let mut guard = state.lock().unwrap();
        let id = get_not_used_in_map_id(&guard.users);
        guard.users.insert(id, name);

        response_data(json!({"id": id}))
    }
    else
    {
        response_error("bad name")
    }
}

fn does_user_belong_to_group(user_id: Id, group_id: Id, user_groups: &HashMap<UserGroupId,UserGroupProps>) -> bool
{
    return user_groups.contains_key(&UserGroupId { user_id, group_id });
}

fn count_admins(group_id: Id, user_groups: &HashMap<UserGroupId, UserGroupProps>) ->usize
{
    let iter = user_groups.into_iter();
    let collection = iter.filter(|&x| x.0.group_id == group_id && x.1.access_level == Access::Admin);
    return collection.count();
}

fn main() -> Result<(), std::io::Error> 
{
    let f = async {
        let data = DataBase
        {
            users: HashMap::new(),
            groups: HashMap::new(),
            user_groups: HashMap::new(),
        };
        let state = Arc::new(Mutex::new(data));
        let mut app = tide::with_state(state);

        // Routes
        app.at("/users")
            .get(|request: Request<Arc<Mutex<DataBase>>>| async move {
                let guard = request.state().lock().unwrap();
                Ok(json!(guard.users))
            });
        app.at("/groups")
            .get(|request: Request<Arc<Mutex<DataBase>>>| async move {
                let guard = request.state().lock().unwrap();
                Ok(json!(guard.groups))
            });
        
        app.at("/user/create")
            .post(|mut request: Request<Arc<Mutex<DataBase>>>| async move {
                let body: Value = request.body_json().await?;
                let input_obj = body.as_object().unwrap();
                Ok(user_create(input_obj, request.state()))
            });
        app.at("/group/create")
            .post(|mut request: Request<Arc<Mutex<DataBase>>>| async move {
                let body: Value = request.body_json().await?;
                let object = body.as_object().unwrap();
                let creator_id: Id = get_field(object, "creator_id");

                let mut guard = request.state().lock().unwrap();
                Ok(if !guard.users.contains_key(&creator_id)
                {
                    response_error("no such user")
                }
                else
                {
                    let id = get_not_used_in_map_id(&guard.groups);
                    guard.groups.insert(id, false);
                    guard.user_groups.insert(
                        UserGroupId
                        {
                            user_id: creator_id,
                            group_id: id,
                        },
                        UserGroupProps
                        {
                            access_level: Access::Admin,
                            santa_id: 0,
                        }
                    );
                    response_data(json!({"group_id": id}))
                })
            });
        app.at("/group/join")
            .post(|mut request: Request<Arc<Mutex<DataBase>>>| async move {
                let value: Value = request.body_json().await.unwrap();
                let object = value.as_object().unwrap();
                let mut user_group_id = UserGroupId{user_id: 0, group_id: 0};
                user_group_id.user_id = get_field(object, "user_id");
                user_group_id.group_id = get_field(object, "group_id");

                let mut guard = request.state().lock().unwrap();
                Ok(match guard.groups.get(&user_group_id.group_id)
                {
                    None => response_error("no such group"),
                    Some(is_closed) =>
                    {
                        if *is_closed
                        {
                            response_error("group is closed")
                        }
                        else
                        {
                            if !guard.users.contains_key(&user_group_id.user_id)
                            {
                                response_error("no such user")
                            }
                            else
                            {
                                if guard.user_groups.contains_key(&user_group_id)
                                {
                                    response_error("user already in group")
                                }
                                else
                                {
                                    guard.user_groups.insert(user_group_id, UserGroupProps{access_level: Access::User, santa_id: 0});
                                    response_empty()
                                }
                            }
                        }
                    },
                })
            });
        app.at("/group/unadmin")
            .post(|mut request: Request<Arc<Mutex<DataBase>>>| async move {
                let body: Value = request.body_json().await?;
                let object = body.as_object().unwrap();
                let admin_id = get_field(object, "admin_id");
                let group_id = get_field(object, "group_id");

                let mut guard = request.state().lock().unwrap();
                Ok(if !does_user_belong_to_group(admin_id, group_id, &guard.user_groups)
                {
                    response_error("User does not belong to this group. Try again.")
                }
                else 
                {
                    let ugid = UserGroupId { user_id: admin_id, group_id: group_id};
                    let ugp = guard.user_groups.get(&ugid).unwrap();
                    if ugp.access_level != Access::Admin
                    {
                        response_error("This user is not an admin.")
                    }
                    else
                    {
                        if count_admins(group_id, &guard.user_groups) < 2
                        {
                            response_error("It is impossible to remove the last admin in a group. You can appoint a new admin and repeat or delete the whole group.")
                        }
                        else
                        {
                            let mut ugp1 = guard.user_groups.get_mut(&ugid).unwrap();
                            ugp1.access_level = Access::User;
                            response_empty()
                        }
                    }
                })
            });
        app.at("/group/delete")
            .post(|mut request: Request<Arc<Mutex<DataBase>>>| async move {
                let body: Value = request.body_json().await?;
                let object = body.as_object().unwrap();
                let admin_id = get_field(object, "admin_id");
                let group_id = get_field(object, "group_id");

                let mut guard = request.state().lock().unwrap();
                Ok(if !does_user_belong_to_group(admin_id, group_id, &guard.user_groups)
                {
                    response_error("User does not belong to this group. Try again.")
                }
                else
                {
                    let ugid = UserGroupId { user_id: admin_id, group_id: group_id};
                    let ugp = guard.user_groups.get(&ugid).unwrap();
                    if ugp.access_level != Access::Admin
                    {
                        response_error("This user is not an admin.")
                    }
                    else
                    {
                        // Before delete group, we need to delete all users from this group
                        guard.user_groups.retain(|user_group_id, _| {
                            user_group_id.group_id != group_id
                        });
                        guard.groups.remove(&group_id);
                        response_empty()
                    }
                }
            )});
        app.listen("127.0.0.1:8080").await
    };
    futures::executor::block_on(f)
}
